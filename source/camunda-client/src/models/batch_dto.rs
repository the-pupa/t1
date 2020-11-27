/*
 * Camunda BPM REST API
 *
 * OpenApi Spec for Camunda BPM REST API.
 *
 * The version of the OpenAPI document: 7.14.0
 * 
 * Generated by: https://openapi-generator.tech
 */




#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BatchDto {
    /// The id of the batch.
    #[serde(rename = "id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The type of the batch.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub _type: Option<String>,
    /// The total jobs of a batch is the number of batch execution jobs required to complete the batch.
    #[serde(rename = "totalJobs", skip_serializing_if = "Option::is_none")]
    pub total_jobs: Option<i32>,
    /// The number of batch execution jobs already created by the seed job.
    #[serde(rename = "jobsCreated", skip_serializing_if = "Option::is_none")]
    pub jobs_created: Option<i32>,
    /// The number of batch execution jobs created per seed job invocation. The batch seed job is invoked until it has created all batch execution jobs required by the batch (see totalJobs property).
    #[serde(rename = "batchJobsPerSeed", skip_serializing_if = "Option::is_none")]
    pub batch_jobs_per_seed: Option<i32>,
    /// Every batch execution job invokes the command executed by the batch invocationsPerBatchJob times. E.g., for a process instance migration batch this specifies the number of process instances which are migrated per batch execution job.
    #[serde(rename = "invocationsPerBatchJob", skip_serializing_if = "Option::is_none")]
    pub invocations_per_batch_job: Option<i32>,
    /// The job definition id for the seed jobs of this batch.
    #[serde(rename = "seedJobDefinitionId", skip_serializing_if = "Option::is_none")]
    pub seed_job_definition_id: Option<String>,
    /// The job definition id for the monitor jobs of this batch.
    #[serde(rename = "monitorJobDefinitionId", skip_serializing_if = "Option::is_none")]
    pub monitor_job_definition_id: Option<String>,
    /// The job definition id for the batch execution jobs of this batch.
    #[serde(rename = "batchJobDefinitionId", skip_serializing_if = "Option::is_none")]
    pub batch_job_definition_id: Option<String>,
    /// Indicates whether this batch is suspended or not.
    #[serde(rename = "suspended", skip_serializing_if = "Option::is_none")]
    pub suspended: Option<bool>,
    /// The tenant id of the batch.
    #[serde(rename = "tenantId", skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    /// The id of the user that created the batch.
    #[serde(rename = "createUserId", skip_serializing_if = "Option::is_none")]
    pub create_user_id: Option<String>,
}

impl BatchDto {
    pub fn new() -> BatchDto {
        BatchDto {
            id: None,
            _type: None,
            total_jobs: None,
            jobs_created: None,
            batch_jobs_per_seed: None,
            invocations_per_batch_job: None,
            seed_job_definition_id: None,
            monitor_job_definition_id: None,
            batch_job_definition_id: None,
            suspended: None,
            tenant_id: None,
            create_user_id: None,
        }
    }
}


