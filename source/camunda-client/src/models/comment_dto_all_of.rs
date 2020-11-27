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
pub struct CommentDtoAllOf {
    /// The id of the task comment.
    #[serde(rename = "id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The id of the user who created the comment.
    #[serde(rename = "userId", skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// The id of the task to which the comment belongs.
    #[serde(rename = "taskId", skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    /// The time when the comment was created. [Default format]($(docsUrl)/reference/rest/overview/date-format/) `yyyy-MM-dd'T'HH:mm:ss.SSSZ`.
    #[serde(rename = "time", skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    /// The content of the comment.
    #[serde(rename = "message", skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// The time after which the comment should be removed by the History Cleanup job. [Default format]($(docsUrl)/reference/rest/overview/date-format/) `yyyy-MM-dd'T'HH:mm:ss.SSSZ`.
    #[serde(rename = "removalTime", skip_serializing_if = "Option::is_none")]
    pub removal_time: Option<String>,
    /// The process instance id of the root process instance that initiated the process containing the task.
    #[serde(rename = "rootProcessInstanceId", skip_serializing_if = "Option::is_none")]
    pub root_process_instance_id: Option<String>,
}

impl CommentDtoAllOf {
    pub fn new() -> CommentDtoAllOf {
        CommentDtoAllOf {
            id: None,
            user_id: None,
            task_id: None,
            time: None,
            message: None,
            removal_time: None,
            root_process_instance_id: None,
        }
    }
}


