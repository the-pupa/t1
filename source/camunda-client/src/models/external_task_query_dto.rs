/*
 * Camunda BPM REST API
 *
 * OpenApi Spec for Camunda BPM REST API.
 *
 * The version of the OpenAPI document: 7.14.0
 * 
 * Generated by: https://openapi-generator.tech
 */

/// ExternalTaskQueryDto : A JSON object with the following properties:



#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExternalTaskQueryDto {
    /// Filter by an external task's id.
    #[serde(rename = "externalTaskId", skip_serializing_if = "Option::is_none")]
    pub external_task_id: Option<String>,
    /// Filter by the comma-separated list of external task ids.
    #[serde(rename = "externalTaskIdIn", skip_serializing_if = "Option::is_none")]
    pub external_task_id_in: Option<Vec<String>>,
    /// Filter by an external task topic.
    #[serde(rename = "topicName", skip_serializing_if = "Option::is_none")]
    pub topic_name: Option<String>,
    /// Filter by the id of the worker that the task was most recently locked by.
    #[serde(rename = "workerId", skip_serializing_if = "Option::is_none")]
    pub worker_id: Option<String>,
    /// Only include external tasks that are currently locked (i.e., they have a lock time and it has not expired). Value may only be `true`, as `false` matches any external task.
    #[serde(rename = "locked", skip_serializing_if = "Option::is_none")]
    pub locked: Option<bool>,
    /// Only include external tasks that are currently not locked (i.e., they have no lock or it has expired). Value may only be `true`, as `false` matches any external task.
    #[serde(rename = "notLocked", skip_serializing_if = "Option::is_none")]
    pub not_locked: Option<bool>,
    /// Only include external tasks that have a positive (&gt; 0) number of retries (or `null`). Value may only be `true`, as `false` matches any external task.
    #[serde(rename = "withRetriesLeft", skip_serializing_if = "Option::is_none")]
    pub with_retries_left: Option<bool>,
    /// Only include external tasks that have 0 retries. Value may only be `true`, as `false` matches any external task.
    #[serde(rename = "noRetriesLeft", skip_serializing_if = "Option::is_none")]
    pub no_retries_left: Option<bool>,
    /// Restrict to external tasks that have a lock that expires after a given date. By [default](https://docs.camunda.org/manual/7.14/reference/rest/overview/date-format/), the date must have the format `yyyy-MM-dd'T'HH:mm:ss.SSSZ`, e.g., `2013-01-23T14:42:45.000+0200`.
    #[serde(rename = "lockExpirationAfter", skip_serializing_if = "Option::is_none")]
    pub lock_expiration_after: Option<String>,
    /// Restrict to external tasks that have a lock that expires before a given date. By [default](https://docs.camunda.org/manual/7.14/reference/rest/overview/date-format/), the date must have the format `yyyy-MM-dd'T'HH:mm:ss.SSSZ`, e.g., `2013-01-23T14:42:45.000+0200`.
    #[serde(rename = "lockExpirationBefore", skip_serializing_if = "Option::is_none")]
    pub lock_expiration_before: Option<String>,
    /// Filter by the id of the activity that an external task is created for.
    #[serde(rename = "activityId", skip_serializing_if = "Option::is_none")]
    pub activity_id: Option<String>,
    /// Filter by the comma-separated list of ids of the activities that an external task is created for.
    #[serde(rename = "activityIdIn", skip_serializing_if = "Option::is_none")]
    pub activity_id_in: Option<Vec<String>>,
    /// Filter by the id of the execution that an external task belongs to.
    #[serde(rename = "executionId", skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<String>,
    /// Filter by the id of the process instance that an external task belongs to.
    #[serde(rename = "processInstanceId", skip_serializing_if = "Option::is_none")]
    pub process_instance_id: Option<String>,
    /// Filter by a comma-separated list of process instance ids that an external task may belong to.
    #[serde(rename = "processInstanceIdIn", skip_serializing_if = "Option::is_none")]
    pub process_instance_id_in: Option<Vec<String>>,
    /// Filter by the id of the process definition that an external task belongs to.
    #[serde(rename = "processDefinitionId", skip_serializing_if = "Option::is_none")]
    pub process_definition_id: Option<String>,
    /// Filter by a comma-separated list of tenant ids. An external task must have one of the given tenant ids.
    #[serde(rename = "tenantIdIn", skip_serializing_if = "Option::is_none")]
    pub tenant_id_in: Option<Vec<String>>,
    /// Only include active tasks. Value may only be `true`, as `false` matches any external task.
    #[serde(rename = "active", skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
    /// Only include suspended tasks. Value may only be `true`, as `false` matches any external task.
    #[serde(rename = "suspended", skip_serializing_if = "Option::is_none")]
    pub suspended: Option<bool>,
    /// Only include jobs with a priority higher than or equal to the given value. Value must be a valid `long` value.
    #[serde(rename = "priorityHigherThanOrEquals", skip_serializing_if = "Option::is_none")]
    pub priority_higher_than_or_equals: Option<i64>,
    /// Only include jobs with a priority lower than or equal to the given value. Value must be a valid `long` value.
    #[serde(rename = "priorityLowerThanOrEquals", skip_serializing_if = "Option::is_none")]
    pub priority_lower_than_or_equals: Option<i64>,
    /// A JSON array of criteria to sort the result by. Each element of the array is a JSON object that                     specifies one ordering. The position in the array identifies the rank of an ordering, i.e., whether                     it is primary, secondary, etc. The ordering objects have the following properties:                      **Note:** The `sorting` properties will not be applied to the External Task count query.
    #[serde(rename = "sorting", skip_serializing_if = "Option::is_none")]
    pub sorting: Option<Vec<crate::models::ExternalTaskQueryDtoSorting>>,
}

impl ExternalTaskQueryDto {
    /// A JSON object with the following properties:
    pub fn new() -> ExternalTaskQueryDto {
        ExternalTaskQueryDto {
            external_task_id: None,
            external_task_id_in: None,
            topic_name: None,
            worker_id: None,
            locked: None,
            not_locked: None,
            with_retries_left: None,
            no_retries_left: None,
            lock_expiration_after: None,
            lock_expiration_before: None,
            activity_id: None,
            activity_id_in: None,
            execution_id: None,
            process_instance_id: None,
            process_instance_id_in: None,
            process_definition_id: None,
            tenant_id_in: None,
            active: None,
            suspended: None,
            priority_higher_than_or_equals: None,
            priority_lower_than_or_equals: None,
            sorting: None,
        }
    }
}


