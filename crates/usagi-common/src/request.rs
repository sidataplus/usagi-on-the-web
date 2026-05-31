use uuid::Uuid;

pub const REQUEST_ID_HEADER: &str = "x-request-id";

pub fn generate_request_id() -> String {
    format!("req_{}", Uuid::new_v4().simple())
}
