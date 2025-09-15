// Test Cases for Early Termination Validation Issues
// This file contains examples of code structures that could be missed
// by the current early termination optimizations in the probe codebase

use std::collections::HashMap;
use serde::{Serialize, Deserialize};

// TEST CASE 1: Multiline Function Signature with Generic Constraints
// Issue: If target lines are [15-20] but function signature spans lines 10-14,
// AST filtering might skip this entire function node
pub fn complex_database_query<T, U, E>(
    connection: &mut DatabaseConnection,
    query_params: QueryParameters<T>,
    result_transformer: impl Fn(RawResult) -> Result<U, E> + Send + Sync,
    retry_config: RetryConfiguration
) -> Result<Vec<U>, DatabaseError<E>>
where
    T: Serialize + Clone + Send + 'static,
    U: Deserialize<'static> + Clone + Debug,
    E: std::error::Error + Send + Sync + 'static
{
    // If search targets these implementation lines but misses the signature above,
    // the semantic context of the function's purpose is lost
    let mut results = Vec::new();
    let mut retry_count = 0;

    loop {
        match connection.execute_query(&query_params) {
            Ok(raw_results) => {
                for raw_result in raw_results {
                    match result_transformer(raw_result) {
                        Ok(transformed) => results.push(transformed),
                        Err(transform_error) => {
                            return Err(DatabaseError::TransformationFailed(transform_error));
                        }
                    }
                }
                break;
            }
            Err(db_error) if retry_count < retry_config.max_retries => {
                retry_count += 1;
                std::thread::sleep(retry_config.delay);
                continue;
            }
            Err(db_error) => return Err(DatabaseError::QueryFailed(db_error)),
        }
    }

    Ok(results)
}

// TEST CASE 2: Complex Trait Implementation with Associated Types
// Issue: The trait bounds and where clauses might place the actual implementation
// outside the initially estimated line ranges
impl<T, R> DataProcessor<T> for ComplexProcessor<T, R>
where
    T: Clone + Debug + Send + Sync + 'static,
    R: ProcessingResult<T> + Clone + Debug,
    T::Error: From<ProcessingError>,
    R::Output: Serialize + DeserializeOwned
{
    type Output = ProcessedData<T, R>;
    type Error = ComplexProcessingError<T::Error>;

    // This implementation might be missed if line range filtering
    // doesn't account for the trait bounds above
    async fn process_batch(
        &self,
        input: Vec<T>,
        config: ProcessingConfig
    ) -> Result<Self::Output, Self::Error> {
        let mut processed_items = Vec::new();
        let processing_context = self.create_context(&config)?;

        for item in input {
            let result = self.process_single_item(item, &processing_context).await?;
            processed_items.push(result);
        }

        Ok(ProcessedData {
            items: processed_items,
            metadata: processing_context.extract_metadata(),
            processing_time: processing_context.elapsed(),
        })
    }

    // Additional methods that form semantic units with the above
    fn validate_input(&self, input: &[T]) -> Result<(), Self::Error> {
        // Validation logic that's semantically related to process_batch
        // but might be in a different "line range" from the perspective of early termination
        for (index, item) in input.iter().enumerate() {
            if !self.is_valid_item(item) {
                return Err(ComplexProcessingError::InvalidInput {
                    index,
                    item: item.clone()
                });
            }
        }
        Ok(())
    }
}

// TEST CASE 3: Macro-Generated Code with Dynamic Patterns
// Issue: Macro expansions create nodes at unexpected line ranges
macro_rules! generate_service_endpoints {
    (
        $(
            $service_name:ident {
                $(
                    $method:ident($($param:ident: $param_type:ty),*) -> $return_type:ty
                    => $endpoint:expr
                ),*
            }
        ),*
    ) => {
        $(  // Each service generates multiple functions
            pub mod $service_name {
                use super::*;

                $(  // Each method becomes a function
                    pub async fn $method(
                        client: &HttpClient,
                        $($param: $param_type),*
                    ) -> Result<$return_type, ApiError> {
                        let url = format!("{}{}", client.base_url(), $endpoint);
                        let response = client
                            .post(&url)
                            .json(&serde_json::json!({
                                $(stringify!($param): $param),*
                            }))
                            .send()
                            .await?;

                        if response.status().is_success() {
                            Ok(response.json().await?)
                        } else {
                            Err(ApiError::RequestFailed(response.status()))
                        }
                    }
                )*
            }
        )*
    };
}

// The macro expansion below creates many functions, but if AST filtering
// targets specific lines within the expansion, it might miss the broader context
generate_service_endpoints! {
    user_service {
        create_user(name: String, email: String) -> User => "/users",
        get_user(id: u64) -> User => "/users/{id}",
        update_user(id: u64, data: UserUpdateData) -> User => "/users/{id}",
        delete_user(id: u64) -> () => "/users/{id}",
        list_users(page: u32, limit: u32) -> Vec<User> => "/users"
    },
    auth_service {
        login(username: String, password: String) -> AuthToken => "/auth/login",
        refresh_token(token: String) -> AuthToken => "/auth/refresh",
        logout(token: String) -> () => "/auth/logout",
        validate_token(token: String) -> bool => "/auth/validate"
    }
}

// TEST CASE 4: Documentation with Embedded Code Examples
/// A complex service that demonstrates various patterns that might be missed
/// by early termination optimizations.
///
/// # Usage Examples
///
/// Basic usage:
/// ```rust
/// let service = ComplexService::new(config);
/// let result = service.process_data(input).await?;
/// ```
///
/// Advanced usage with custom transformers:
/// ```rust
/// let service = ComplexService::builder()
///     .with_custom_transformer(|data| {
///         // Complex transformation logic
///         transform_data_with_validation(data)
///     })
///     .with_retry_policy(RetryPolicy::exponential_backoff())
///     .with_timeout(Duration::from_secs(30))
///     .build()?;
///
/// let processed = service
///     .process_batch(input_data)
///     .with_context("batch_processing")
///     .await?;
/// ```
///
/// Error handling patterns:
/// ```rust
/// match service.process_data(risky_input).await {
///     Ok(result) => handle_success(result),
///     Err(ProcessingError::ValidationFailed(details)) => {
///         log::warn!("Validation failed: {:?}", details);
///         fallback_processing(risky_input).await?
///     },
///     Err(ProcessingError::TimeoutError) => {
///         log::error!("Processing timed out");
///         Err(ServiceError::ProcessingTimeout)
///     },
///     Err(other_error) => {
///         log::error!("Unexpected error: {:?}", other_error);
///         Err(ServiceError::UnexpectedError(other_error))
///     }
/// }
/// ```
pub struct ComplexService<T, R> {
    processor: Arc<dyn DataProcessor<T, Output = R> + Send + Sync>,
    config: ServiceConfiguration,
    metrics: Arc<Mutex<ServiceMetrics>>,
}

// The implementation methods below might be missed if the search targets
// the documentation examples above, but AST filtering only considers
// the method implementation line ranges
impl<T, R> ComplexService<T, R>
where
    T: Send + Sync + 'static,
    R: Send + Sync + 'static
{
    /// Creates a new service instance with the provided configuration.
    /// This method demonstrates how semantic relationships can span multiple
    /// "optimization boundaries" in the current early termination logic.
    pub fn new(config: ServiceConfiguration) -> Result<Self, ServiceCreationError> {
        let processor = Self::create_default_processor(&config)?;
        let metrics = Arc::new(Mutex::new(ServiceMetrics::new()));

        Ok(ComplexService {
            processor,
            config,
            metrics,
        })
    }

    /// Process data with comprehensive error handling and metrics collection.
    /// This method contains patterns that might be split across different
    /// line ranges from an AST filtering perspective.
    pub async fn process_data(
        &self,
        input: ProcessingInput<T>
    ) -> Result<ProcessingOutput<R>, ProcessingError> {
        let start_time = Instant::now();
        let processing_id = Uuid::new_v4();

        // Pre-processing validation - might be in different "optimization zone"
        self.validate_processing_input(&input)
            .map_err(|e| ProcessingError::ValidationFailed {
                processing_id,
                details: e.to_string()
            })?;

        // Main processing logic - could be separated by line range filtering
        let processing_context = ProcessingContext::new(processing_id, &self.config);
        let intermediate_result = self
            .processor
            .process_with_context(input, processing_context)
            .await
            .map_err(|e| ProcessingError::ProcessorFailed {
                processing_id,
                source: e
            })?;

        // Post-processing and metrics - might be in yet another "zone"
        let final_result = self
            .apply_post_processing_rules(intermediate_result)
            .await?;

        self.update_processing_metrics(processing_id, start_time.elapsed())
            .await;

        Ok(final_result)
    }

    // Helper methods that form semantic units but might be missed
    // by line-range-based early termination
    fn validate_processing_input(
        &self,
        input: &ProcessingInput<T>
    ) -> Result<(), ValidationError> {
        // Validation logic that's semantically connected to process_data
        // but might be filtered out if not in the target line ranges
        if input.data.is_empty() {
            return Err(ValidationError::EmptyInput);
        }

        if input.data.len() > self.config.max_batch_size {
            return Err(ValidationError::BatchTooLarge {
                size: input.data.len(),
                max_allowed: self.config.max_batch_size
            });
        }

        Ok(())
    }

    async fn apply_post_processing_rules(
        &self,
        intermediate: IntermediateResult<R>
    ) -> Result<ProcessingOutput<R>, ProcessingError> {
        // Post-processing logic that completes the semantic picture
        // but might be separated from the main processing logic
        // by AST node boundary calculations
        let mut output = ProcessingOutput::from_intermediate(intermediate);

        for rule in &self.config.post_processing_rules {
            output = rule.apply(output).await?;
        }

        output.finalize_with_metadata(&self.config);
        Ok(output)
    }
}

// TEST CASE 5: Complex Generic Constraints That Span Multiple Lines
// Issue: The where clause and generic bounds might be considered separate
// from the implementation by simple line intersection checks
pub struct GenericRepository<T, K, E>
where
    T: Entity<Key = K> + Serialize + DeserializeOwned + Clone + Debug + Send + Sync + 'static,
    K: EntityKey + Hash + Eq + Clone + Debug + Send + Sync + 'static,
    E: RepositoryError + From<DatabaseError> + Send + Sync + 'static,
    T::QueryBuilder: QueryBuilder<T, K> + Send + Sync,
    T::Validator: EntityValidator<T> + Send + Sync,
{
    connection_pool: Arc<ConnectionPool>,
    query_cache: Arc<RwLock<HashMap<K, CachedQuery<T>>>>,
    validation_rules: Vec<Box<dyn ValidationRule<T> + Send + Sync>>,
    metrics_collector: Option<Arc<dyn MetricsCollector + Send + Sync>>,
    _phantom: PhantomData<(T, K, E)>,
}

// The implementation methods might be missed if the search focuses on specific
// method bodies but the generic constraints above define the semantic context
impl<T, K, E> GenericRepository<T, K, E>
where
    T: Entity<Key = K> + Serialize + DeserializeOwned + Clone + Debug + Send + Sync + 'static,
    K: EntityKey + Hash + Eq + Clone + Debug + Send + Sync + 'static,
    E: RepositoryError + From<DatabaseError> + Send + Sync + 'static,
    T::QueryBuilder: QueryBuilder<T, K> + Send + Sync,
    T::Validator: EntityValidator<T> + Send + Sync,
{
    // This method's implementation might be targeted by search, but without
    // the generic constraints context above, the semantic meaning is incomplete
    pub async fn find_with_complex_criteria(
        &self,
        criteria: ComplexQueryCriteria<T, K>
    ) -> Result<PaginatedResult<T>, E> {
        // Method implementation that relies heavily on the generic constraints
        // defined above but might be filtered separately by line range optimization
        let query_builder = T::QueryBuilder::new();
        let validated_criteria = T::Validator::validate_query_criteria(&criteria)
            .map_err(|validation_error| E::from(DatabaseError::ValidationFailed(validation_error)))?;

        let paginated_query = query_builder
            .with_criteria(validated_criteria)
            .with_pagination(criteria.pagination.clone())
            .with_ordering(criteria.sort_options.clone())
            .build()?;

        let results = self.execute_paginated_query(paginated_query).await?;
        Ok(results)
    }
}

// TEST CASE 6: Nested Module Structure with Cross-References
pub mod complex_nested_structure {
    use super::*;

    pub mod data_access {
        use super::*;

        // This function references types and functions defined in other modules
        // Early termination might miss these cross-module dependencies
        pub async fn load_user_with_permissions(
            user_id: u64,
            repository: &impl UserRepository,
            permission_service: &impl PermissionService
        ) -> Result<UserWithPermissions, DataAccessError> {
            // The function body references external services and types
            // If search targets this implementation but misses the service interfaces,
            // the complete semantic picture is lost
            let user = repository.find_by_id(user_id).await?
                .ok_or(DataAccessError::UserNotFound(user_id))?;

            let permissions = permission_service
                .get_user_permissions(user_id)
                .await
                .map_err(DataAccessError::PermissionServiceError)?;

            let role_permissions = permission_service
                .get_role_permissions(&user.role)
                .await
                .map_err(DataAccessError::PermissionServiceError)?;

            Ok(UserWithPermissions {
                user,
                direct_permissions: permissions,
                role_permissions,
                computed_at: Utc::now(),
            })
        }
    }

    pub mod business_logic {
        use super::*;
        use super::data_access::*;

        // This service uses the data access functions above
        // but might be considered in a separate "optimization zone"
        pub struct UserManagementService {
            user_repository: Arc<dyn UserRepository + Send + Sync>,
            permission_service: Arc<dyn PermissionService + Send + Sync>,
            audit_logger: Arc<dyn AuditLogger + Send + Sync>,
        }

        impl UserManagementService {
            // This method creates semantic relationships with the data_access module
            // but early termination might not capture these relationships
            pub async fn update_user_with_permission_check(
                &self,
                user_id: u64,
                updates: UserUpdateRequest,
                requesting_user_id: u64
            ) -> Result<User, UserManagementError> {
                // Load the requesting user with permissions (cross-module dependency)
                let requesting_user = load_user_with_permissions(
                    requesting_user_id,
                    self.user_repository.as_ref(),
                    self.permission_service.as_ref()
                ).await
                .map_err(UserManagementError::DataAccessError)?;

                // Permission check logic that spans multiple semantic boundaries
                if !requesting_user.can_modify_user(user_id) {
                    self.audit_logger.log_unauthorized_access(
                        requesting_user_id,
                        "update_user",
                        user_id
                    ).await;
                    return Err(UserManagementError::InsufficientPermissions);
                }

                // The actual update logic references many external components
                let updated_user = self.user_repository
                    .update(user_id, updates)
                    .await
                    .map_err(UserManagementError::RepositoryError)?;

                self.audit_logger.log_user_update(
                    requesting_user_id,
                    user_id,
                    &updates
                ).await;

                Ok(updated_user)
            }
        }
    }
}

// Dummy types and traits for compilation
trait Entity {
    type Key;
    type QueryBuilder;
    type Validator;
}

trait EntityKey {}
trait RepositoryError {}
trait EntityValidator<T> {
    fn validate_query_criteria<K>(criteria: &ComplexQueryCriteria<T, K>) -> Result<(), String>;
}
trait QueryBuilder<T, K> {
    fn new() -> Self;
    fn with_criteria(self, criteria: ValidatedCriteria<T, K>) -> Self;
    fn with_pagination(self, pagination: PaginationOptions) -> Self;
    fn with_ordering(self, sort: SortOptions) -> Self;
    fn build(self) -> Result<PaginatedQuery<T>, DatabaseError>;
}

// Additional supporting types...
struct DatabaseConnection;
struct QueryParameters<T>(T);
struct RetryConfiguration { max_retries: u32, delay: std::time::Duration }
struct DatabaseError<E>(E);
struct RawResult;
struct ProcessedData<T, R> { items: Vec<T>, metadata: String, processing_time: std::time::Duration }
struct ProcessingConfig;
struct ComplexProcessingError<E>(E);
struct ProcessingContext;
struct HttpClient { base_url: String }
struct ApiError;
struct ServiceConfiguration { max_batch_size: usize, post_processing_rules: Vec<PostProcessingRule> }
struct ServiceMetrics;
struct ServiceCreationError;
struct ProcessingInput<T> { data: Vec<T> }
struct ProcessingOutput<R>(R);
struct ProcessingError;
struct ValidationError;
struct IntermediateResult<R>(R);
struct PostProcessingRule;
struct ConnectionPool;
struct CachedQuery<T>(T);
trait ValidationRule<T> {}
trait MetricsCollector {}
struct ComplexQueryCriteria<T, K> { pagination: PaginationOptions, sort_options: SortOptions, _phantom: PhantomData<(T, K)> }
struct PaginatedResult<T>(Vec<T>);
struct ValidatedCriteria<T, K>(PhantomData<(T, K)>);
struct PaginationOptions;
struct SortOptions;
struct PaginatedQuery<T>(T);
struct DataAccessError;
trait UserRepository {
    async fn find_by_id(&self, id: u64) -> Result<Option<User>, DatabaseError<String>>;
    async fn update(&self, id: u64, updates: UserUpdateRequest) -> Result<User, DatabaseError<String>>;
}
trait PermissionService {
    async fn get_user_permissions(&self, user_id: u64) -> Result<Vec<Permission>, String>;
    async fn get_role_permissions(&self, role: &str) -> Result<Vec<Permission>, String>;
}
struct UserWithPermissions { user: User, direct_permissions: Vec<Permission>, role_permissions: Vec<Permission>, computed_at: chrono::DateTime<chrono::Utc> }
struct User { role: String }
struct Permission;
struct UserManagementError;
struct UserUpdateRequest;
trait AuditLogger {
    async fn log_unauthorized_access(&self, user_id: u64, action: &str, target_id: u64);
    async fn log_user_update(&self, requesting_user_id: u64, target_user_id: u64, updates: &UserUpdateRequest);
}