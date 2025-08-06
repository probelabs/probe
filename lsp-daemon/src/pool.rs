use crate::language_detector::Language;
use crate::lsp_registry::LspServerConfig;
use crate::lsp_server::LspServer;
use anyhow::Result;
use dashmap::DashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use tokio::time::{Duration, Instant};
use tracing::{debug, info, warn};
use uuid::Uuid;

#[derive(Clone)]
pub struct PooledServer {
    pub id: Uuid,
    pub server: Arc<LspServer>,
    pub last_used: Instant,
    pub request_count: usize,
}

#[derive(Clone)]
pub struct LspServerPool {
    config: Arc<LspServerConfig>,
    ready: Arc<Mutex<VecDeque<PooledServer>>>,
    busy: Arc<DashMap<Uuid, PooledServer>>,
    semaphore: Arc<Semaphore>,
    min_size: usize,
    max_size: usize,
    max_requests_per_server: usize,
}

impl LspServerPool {
    pub fn new(config: LspServerConfig) -> Self {
        let min_size = 1;
        let max_size = 4;
        let pool = Self {
            config: Arc::new(config),
            ready: Arc::new(Mutex::new(VecDeque::new())),
            busy: Arc::new(DashMap::new()),
            semaphore: Arc::new(Semaphore::new(max_size)),
            min_size,
            max_size,
            max_requests_per_server: 100, // Recycle after 100 requests
        };

        // Start warming minimum servers
        let pool_clone = pool.clone();
        tokio::spawn(async move {
            pool_clone.ensure_minimum_servers().await;
        });

        pool
    }

    pub async fn ensure_minimum_servers(&self) {
        let ready_count = self.ready.lock().await.len();
        let busy_count = self.busy.len();
        let total = ready_count + busy_count;

        if total < self.min_size {
            let needed = self.min_size - total;
            info!(
                "Pool for {:?}: Starting {} servers (current: {}, min: {})",
                self.config.language, needed, total, self.min_size
            );

            for _ in 0..needed {
                let config = self.config.clone();
                let ready = self.ready.clone();

                tokio::spawn(async move {
                    match Self::spawn_server(&config).await {
                        Ok(server) => {
                            let pooled = PooledServer {
                                id: Uuid::new_v4(),
                                server: Arc::new(server),
                                last_used: Instant::now(),
                                request_count: 0,
                            };
                            ready.lock().await.push_back(pooled);
                            info!(
                                "Successfully spawned and warmed LSP server for {:?}",
                                config.language
                            );
                        }
                        Err(e) => {
                            warn!(
                                "Failed to spawn LSP server for {:?}: {}",
                                config.language, e
                            );
                        }
                    }
                });
            }
        }
    }

    async fn spawn_server(config: &LspServerConfig) -> Result<LspServer> {
        debug!("Spawning new LSP server for {:?}", config.language);
        let mut server = LspServer::spawn(config)?;
        server.initialize(config).await?;
        server.wait_until_ready().await?;
        Ok(server)
    }

    pub async fn get_server(&self) -> Result<PooledServer> {
        // Try to get a ready server
        if let Some(server) = self.ready.lock().await.pop_front() {
            debug!(
                "Reusing ready server {} for {:?}",
                server.id, self.config.language
            );

            // Move to busy
            self.busy.insert(server.id, server.clone());

            // Ensure minimum servers in background
            let pool = self.clone();
            tokio::spawn(async move {
                pool.ensure_minimum_servers().await;
            });

            return Ok(server);
        }

        // Check if we can spawn a new server
        if self.busy.len() < self.max_size {
            info!(
                "No ready servers for {:?}, spawning new one",
                self.config.language
            );

            // Acquire semaphore permit
            let _permit = self.semaphore.acquire().await?;

            let server = Self::spawn_server(&self.config).await?;
            let pooled = PooledServer {
                id: Uuid::new_v4(),
                server: Arc::new(server),
                last_used: Instant::now(),
                request_count: 0,
            };

            let pooled_copy = pooled.clone();
            self.busy.insert(pooled.id, pooled_copy);

            return Ok(pooled);
        }

        // Wait for a server to become available
        info!(
            "Pool for {:?} is at capacity, waiting for available server",
            self.config.language
        );

        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;

            if let Some(server) = self.ready.lock().await.pop_front() {
                let server_copy = server.clone();
                self.busy.insert(server.id, server_copy);
                return Ok(server);
            }
        }
    }

    pub async fn return_server(&self, mut server: PooledServer) {
        // Remove from busy
        self.busy.remove(&server.id);

        server.last_used = Instant::now();
        server.request_count += 1;

        // Check if server should be recycled
        if server.request_count >= self.max_requests_per_server {
            info!(
                "Recycling server {} for {:?} after {} requests",
                server.id, self.config.language, server.request_count
            );

            // Shutdown old server
            let _ = server.server.shutdown().await;

            // Spawn replacement in background
            let config = self.config.clone();
            let ready = self.ready.clone();

            tokio::spawn(async move {
                match Self::spawn_server(&config).await {
                    Ok(new_server) => {
                        let pooled = PooledServer {
                            id: Uuid::new_v4(),
                            server: Arc::new(new_server),
                            last_used: Instant::now(),
                            request_count: 0,
                        };
                        ready.lock().await.push_back(pooled);
                    }
                    Err(e) => {
                        warn!("Failed to spawn replacement server: {}", e);
                    }
                }
            });
        } else {
            // Return to ready pool
            self.ready.lock().await.push_back(server);
        }
    }

    pub async fn shutdown(&self) {
        info!("Shutting down pool for {:?}", self.config.language);

        // Shutdown all ready servers
        let mut ready = self.ready.lock().await;
        while let Some(server) = ready.pop_front() {
            let _ = server.server.shutdown().await;
        }

        // Note: Busy servers will be shut down when returned
    }

    pub async fn get_stats(&self) -> PoolStats {
        let ready_count = self.ready.lock().await.len();
        let busy_count = self.busy.len();

        PoolStats {
            language: self.config.language,
            ready_servers: ready_count,
            busy_servers: busy_count,
            total_servers: ready_count + busy_count,
            min_size: self.min_size,
            max_size: self.max_size,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PoolStats {
    pub language: Language,
    pub ready_servers: usize,
    pub busy_servers: usize,
    pub total_servers: usize,
    pub min_size: usize,
    pub max_size: usize,
}

pub struct PoolManager {
    pools: Arc<DashMap<Language, LspServerPool>>,
}

impl PoolManager {
    pub fn new() -> Self {
        Self {
            pools: Arc::new(DashMap::new()),
        }
    }

    pub async fn get_pool(&self, language: Language, config: LspServerConfig) -> LspServerPool {
        self.pools
            .entry(language)
            .or_insert_with(|| LspServerPool::new(config))
            .clone()
    }

    pub async fn shutdown_all(&self) {
        for pool in self.pools.iter() {
            pool.shutdown().await;
        }
        self.pools.clear();
    }

    pub async fn get_all_stats(&self) -> Vec<PoolStats> {
        let mut stats = Vec::new();
        for pool in self.pools.iter() {
            stats.push(pool.get_stats().await);
        }
        stats.sort_by_key(|s| s.language.as_str().to_string());
        stats
    }
}
