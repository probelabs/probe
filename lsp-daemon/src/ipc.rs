use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

// Re-export platform-specific types
#[cfg(unix)]
pub use unix_impl::{IpcListener, IpcStream, OwnedReadHalf, OwnedWriteHalf};

#[cfg(windows)]
pub use windows_impl::{IpcListener, IpcStream, OwnedReadHalf, OwnedWriteHalf};

/// Trait for platform-agnostic IPC listener
#[async_trait]
pub trait IpcListenerTrait: Send + Sync {
    type Stream: IpcStreamTrait;

    async fn accept(&self) -> Result<Self::Stream>;
    fn local_addr(&self) -> Result<String>;
}

/// Trait for platform-agnostic IPC stream
pub trait IpcStreamTrait: AsyncRead + AsyncWrite + Send + Sync + Unpin {
    fn peer_addr(&self) -> Result<String>;
}

// Unix implementation
#[cfg(unix)]
mod unix_impl {
    use super::*;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    use crate::socket_path;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    use socket2::{Domain, Socket, Type};
    #[cfg(any(target_os = "linux", target_os = "android"))]
    use std::io;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    use std::mem::{size_of, zeroed};
    #[cfg(any(target_os = "linux", target_os = "android"))]
    use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd};
    #[cfg(any(target_os = "linux", target_os = "android"))]
    use std::os::unix::net::{UnixListener as StdUnixListener, UnixStream as StdUnixStream};
    use std::path::Path;
    use std::time::Duration;
    use tokio::net::{UnixListener as TokioUnixListener, UnixStream as TokioUnixStream};

    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn create_abstract_addr(name: &[u8]) -> io::Result<(libc::sockaddr_un, libc::socklen_t)> {
        let mut addr: libc::sockaddr_un = unsafe { zeroed() };
        addr.sun_family = libc::AF_UNIX as libc::sa_family_t;
        let max_len = addr.sun_path.len();
        if name.len() + 1 > max_len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "abstract socket name too long",
            ));
        }
        addr.sun_path[0] = 0;
        for (idx, byte) in name.iter().enumerate() {
            addr.sun_path[idx + 1] = *byte as libc::c_char;
        }

        let len = (size_of::<libc::sa_family_t>() + 1 + name.len()) as libc::socklen_t;

        Ok((addr, len))
    }

    pub struct IpcListener {
        listener: TokioUnixListener,
        path: String,
    }

    impl IpcListener {
        pub async fn bind(path: &str) -> Result<Self> {
            Self::bind_internal(path).await
        }

        async fn bind_internal(path: &str) -> Result<Self> {
            #[cfg(any(target_os = "linux", target_os = "android"))]
            if let Some(name) = socket_path::unix_abstract_name(path) {
                // Try abstract bind; on any failure, log and fall back to filesystem socket
                match (|| {
                    let (addr, len) = create_abstract_addr(&name).map_err(|e| {
                        anyhow!("Failed to construct abstract socket address: {}", e)
                    })?;
                    let socket = Socket::new(Domain::UNIX, Type::STREAM, None)
                        .map_err(|e| anyhow!("Failed to create abstract socket: {}", e))?;
                    socket
                        .set_cloexec(true)
                        .map_err(|e| anyhow!("Failed to set CLOEXEC on abstract socket: {}", e))?;
                    let bind_result = unsafe {
                        libc::bind(
                            socket.as_raw_fd(),
                            &addr as *const _ as *const libc::sockaddr,
                            len,
                        )
                    };
                    if bind_result != 0 {
                        return Err(anyhow!(
                            "Failed to bind abstract socket: {}",
                            io::Error::last_os_error()
                        ));
                    }
                    if unsafe { libc::listen(socket.as_raw_fd(), 256) } != 0 {
                        return Err(anyhow!(
                            "Failed to listen on abstract socket: {}",
                            io::Error::last_os_error()
                        ));
                    }
                    if unsafe { libc::fcntl(socket.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) }
                        != 0
                    {
                        return Err(anyhow!(
                            "Failed to set nonblocking on abstract socket: {}",
                            io::Error::last_os_error()
                        ));
                    }
                    let fd = socket.into_raw_fd();
                    let std_listener = unsafe { StdUnixListener::from_raw_fd(fd) };
                    let listener = TokioUnixListener::from_std(std_listener).map_err(|e| {
                        anyhow!("Failed to integrate abstract listener with Tokio: {}", e)
                    })?;
                    Ok(Self {
                        listener,
                        path: path.to_string(),
                    })
                })() {
                    Ok(l) => return Ok(l),
                    Err(e) => {
                        tracing::warn!(
                            "Abstract socket bind failed ({}); falling back to filesystem socket {}",
                            e, path
                        );
                        // fall through to filesystem bind below
                    }
                }
            }

            // Check if socket file exists and if a daemon is listening
            if Path::new(path).exists() {
                // Try to connect to see if a daemon is actually running
                match TokioUnixStream::connect(path).await {
                    Ok(_) => {
                        // Another daemon is running on this socket
                        return Err(anyhow!(
                            "Socket {} is already in use by another daemon",
                            path
                        ));
                    }
                    Err(_) => {
                        // Socket file exists but no daemon is listening (stale socket)
                        tracing::info!("Removing stale socket file: {}", path);
                        std::fs::remove_file(path)?;
                    }
                }
            }

            // Create parent directory if needed
            if let Some(parent) = Path::new(path).parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Bind the socket - this is now protected by our exclusive lock
            let listener = match TokioUnixListener::bind(path) {
                Ok(l) => l,
                Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                    // This shouldn't happen with our locking, but handle it gracefully
                    tracing::warn!(
                        "Socket bind failed due to address in use, retrying after delay"
                    );
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    TokioUnixListener::bind(path)?
                }
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                    return Err(anyhow!(
                        "Permission denied binding UNIX socket at {}. This environment may restrict creating UNIX sockets; set PROBE_LSP_SOCKET_PATH to an allowed location or run outside the sandbox.",
                        path
                    ));
                }
                Err(e) => return Err(e.into()),
            };

            Ok(Self {
                listener,
                path: path.to_string(),
            })
        }

        pub async fn accept(&self) -> Result<IpcStream> {
            let (stream, _) = self.listener.accept().await?;
            Ok(IpcStream { stream })
        }

        pub fn local_addr(&self) -> Result<String> {
            Ok(self.path.clone())
        }
    }

    impl Drop for IpcListener {
        fn drop(&mut self) {
            #[cfg(any(target_os = "linux", target_os = "android"))]
            if socket_path::unix_abstract_name(&self.path).is_some() {
                return;
            }
            // Clean up socket file
            if let Err(e) = std::fs::remove_file(&self.path) {
                // Only log at trace level since this is cleanup code and the file might not exist
                tracing::trace!("Failed to remove socket file during cleanup {}: {} (this is usually not a problem)", self.path, e);
            } else {
                tracing::trace!("Successfully cleaned up socket file: {}", self.path);
            }
        }
    }

    pub struct IpcStream {
        stream: TokioUnixStream,
    }

    impl IpcStream {
        pub async fn connect(path: &str) -> Result<Self> {
            #[cfg(any(target_os = "linux", target_os = "android"))]
            if let Some(name) = socket_path::unix_abstract_name(path) {
                // Try abstract connect; on failure, fall back to filesystem connect
                match (|| {
                    let (addr, len) = create_abstract_addr(&name).map_err(|e| {
                        anyhow!("Failed to construct abstract socket address: {}", e)
                    })?;
                    let socket = Socket::new(Domain::UNIX, Type::STREAM, None)
                        .map_err(|e| anyhow!("Failed to create abstract stream socket: {}", e))?;
                    socket.set_cloexec(true).map_err(|e| {
                        anyhow!("Failed to set CLOEXEC on abstract stream socket: {}", e)
                    })?;
                    let connect_result = unsafe {
                        libc::connect(
                            socket.as_raw_fd(),
                            &addr as *const _ as *const libc::sockaddr,
                            len,
                        )
                    };
                    if connect_result != 0 {
                        let err = io::Error::last_os_error();
                        return Err(anyhow!("Failed to connect to abstract socket: {}", err));
                    }
                    if unsafe { libc::fcntl(socket.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) }
                        != 0
                    {
                        return Err(anyhow!(
                            "Failed to set nonblocking on abstract stream: {}",
                            io::Error::last_os_error()
                        ));
                    }
                    let fd = socket.into_raw_fd();
                    let std_stream = unsafe { StdUnixStream::from_raw_fd(fd) };
                    let stream = TokioUnixStream::from_std(std_stream).map_err(|e| {
                        anyhow!("Failed to integrate abstract stream with Tokio: {}", e)
                    })?;
                    Ok(Self { stream })
                })() {
                    Ok(s) => return Ok(s),
                    Err(e) => {
                        tracing::warn!(
                            "Abstract socket connect failed ({}); falling back to filesystem socket {}",
                            e, path
                        );
                    }
                }
            }

            let stream = TokioUnixStream::connect(path).await?;
            Ok(Self { stream })
        }

        pub fn peer_addr(&self) -> Result<String> {
            Ok("unix-peer".to_string()) // Unix sockets don't have traditional addresses
        }

        pub fn into_split(self) -> (OwnedReadHalf, OwnedWriteHalf) {
            let (reader, writer) = self.stream.into_split();
            (
                OwnedReadHalf { inner: reader },
                OwnedWriteHalf { inner: writer },
            )
        }
    }

    pub struct OwnedReadHalf {
        inner: tokio::net::unix::OwnedReadHalf,
    }

    pub struct OwnedWriteHalf {
        inner: tokio::net::unix::OwnedWriteHalf,
    }

    impl AsyncRead for OwnedReadHalf {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            Pin::new(&mut self.inner).poll_read(cx, buf)
        }
    }

    impl AsyncWrite for OwnedWriteHalf {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            Pin::new(&mut self.inner).poll_write(cx, buf)
        }

        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Pin::new(&mut self.inner).poll_flush(cx)
        }

        fn poll_shutdown(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<std::io::Result<()>> {
            Pin::new(&mut self.inner).poll_shutdown(cx)
        }
    }

    impl AsyncRead for IpcStream {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            Pin::new(&mut self.stream).poll_read(cx, buf)
        }
    }

    impl AsyncWrite for IpcStream {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            Pin::new(&mut self.stream).poll_write(cx, buf)
        }

        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Pin::new(&mut self.stream).poll_flush(cx)
        }

        fn poll_shutdown(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<std::io::Result<()>> {
            Pin::new(&mut self.stream).poll_shutdown(cx)
        }
    }

    impl IpcStreamTrait for IpcStream {
        fn peer_addr(&self) -> Result<String> {
            self.peer_addr()
        }
    }
}

// Windows implementation
#[cfg(windows)]
mod windows_impl {
    use super::*;
    use std::sync::Arc;
    use tokio::net::windows::named_pipe::{
        ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
    };
    use tokio::sync::Mutex;
    use tracing;

    pub struct IpcListener {
        path: String,
        current_server: Arc<Mutex<Option<NamedPipeServer>>>,
    }

    impl IpcListener {
        pub async fn bind(path: &str) -> Result<Self> {
            // Create the first server instance
            let server = ServerOptions::new()
                .first_pipe_instance(true)
                .in_buffer_size(65536)
                .out_buffer_size(65536)
                .create(path)?;

            Ok(Self {
                path: path.to_string(),
                current_server: Arc::new(Mutex::new(Some(server))),
            })
        }
    }

    impl Drop for IpcListener {
        fn drop(&mut self) {
            // Log cleanup action
            tracing::debug!("Cleaning up Windows named pipe: {}", self.path);

            // Named pipes on Windows are automatically cleaned up when the last handle is closed
            // The Tokio NamedPipeServer will handle the cleanup when it's dropped
            // We just need to ensure any remaining server instance is dropped
            if let Ok(mut server_guard) = self.current_server.try_lock() {
                if server_guard.take().is_some() {
                    tracing::debug!(
                        "Closed remaining named pipe server instance for: {}",
                        self.path
                    );
                }
            } else {
                tracing::warn!(
                    "Could not acquire lock to clean up named pipe server: {}",
                    self.path
                );
            }
        }
    }

    impl IpcListener {
        pub async fn accept(&self) -> Result<IpcStream> {
            let mut server_guard = self.current_server.lock().await;

            if let Some(server) = server_guard.take() {
                // Wait for a client to connect
                server.connect().await?;

                // Create a new server instance for the next connection
                let new_server = ServerOptions::new()
                    .first_pipe_instance(false)
                    .in_buffer_size(65536)
                    .out_buffer_size(65536)
                    .create(&self.path)?;
                *server_guard = Some(new_server);

                // Return the connected server as a stream
                // Windows named pipes work bidirectionally, so the server pipe
                // can be used for both reading and writing after connection
                Ok(IpcStream {
                    stream: IpcStreamInner::Server(server),
                })
            } else {
                Err(anyhow!("No server available"))
            }
        }

        pub fn local_addr(&self) -> Result<String> {
            Ok(self.path.clone())
        }
    }

    enum IpcStreamInner {
        Client(NamedPipeClient),
        Server(NamedPipeServer),
    }

    pub struct IpcStream {
        stream: IpcStreamInner,
    }

    impl IpcStream {
        pub async fn connect(path: &str) -> Result<Self> {
            let client = ClientOptions::new().open(path)?;

            Ok(Self {
                stream: IpcStreamInner::Client(client),
            })
        }

        pub fn peer_addr(&self) -> Result<String> {
            Ok("windows-pipe-peer".to_string())
        }

        pub fn into_split(self) -> (OwnedReadHalf, OwnedWriteHalf) {
            let stream = Arc::new(Mutex::new(self));
            (
                OwnedReadHalf {
                    stream: stream.clone(),
                },
                OwnedWriteHalf { stream },
            )
        }
    }

    pub struct OwnedReadHalf {
        stream: Arc<Mutex<IpcStream>>,
    }

    pub struct OwnedWriteHalf {
        stream: Arc<Mutex<IpcStream>>,
    }

    impl AsyncRead for OwnedReadHalf {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            let mut stream = match self.stream.try_lock() {
                Ok(guard) => guard,
                Err(_) => {
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                }
            };
            Pin::new(&mut *stream).poll_read(cx, buf)
        }
    }

    impl AsyncWrite for OwnedWriteHalf {
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            let mut stream = match self.stream.try_lock() {
                Ok(guard) => guard,
                Err(_) => {
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                }
            };
            Pin::new(&mut *stream).poll_write(cx, buf)
        }

        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            let mut stream = match self.stream.try_lock() {
                Ok(guard) => guard,
                Err(_) => {
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                }
            };
            Pin::new(&mut *stream).poll_flush(cx)
        }

        fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            let mut stream = match self.stream.try_lock() {
                Ok(guard) => guard,
                Err(_) => {
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                }
            };
            Pin::new(&mut *stream).poll_shutdown(cx)
        }
    }

    impl AsyncRead for IpcStream {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            match &mut self.stream {
                IpcStreamInner::Client(client) => Pin::new(client).poll_read(cx, buf),
                IpcStreamInner::Server(server) => Pin::new(server).poll_read(cx, buf),
            }
        }
    }

    impl AsyncWrite for IpcStream {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            match &mut self.stream {
                IpcStreamInner::Client(client) => Pin::new(client).poll_write(cx, buf),
                IpcStreamInner::Server(server) => Pin::new(server).poll_write(cx, buf),
            }
        }

        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            match &mut self.stream {
                IpcStreamInner::Client(client) => Pin::new(client).poll_flush(cx),
                IpcStreamInner::Server(server) => Pin::new(server).poll_flush(cx),
            }
        }

        fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            // Named pipes don't have a shutdown method, so we just flush
            self.poll_flush(cx)
        }
    }

    impl IpcStreamTrait for IpcStream {
        fn peer_addr(&self) -> Result<String> {
            self.peer_addr()
        }
    }
}

/// Helper function to create an IPC listener
pub async fn bind(path: &str) -> Result<IpcListener> {
    IpcListener::bind(path).await
}

/// Helper function to connect to an IPC endpoint
pub async fn connect(path: &str) -> Result<IpcStream> {
    IpcStream::connect(path).await
}
