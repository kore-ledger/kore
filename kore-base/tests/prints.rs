// 
use identity::identifier::DigestIdentifier;
use kore_base::{
    model::{
        namespace::Namespace,
        request::{EventRequest, CreateRequest}
    },
};

/// Print event create governance request
#[test]
fn test_event_request_creation() {
    let event = EventRequest::Create(CreateRequest {
            name: Some("governance_name".to_owned()),
            description: Some("governance_description".to_owned()),
            governance_id: DigestIdentifier::default(),
            schema_id: "schema_id".to_owned(),
            namespace: Namespace::from("kc"),
        },        
    );

    let text = serde_json::to_string_pretty(&event).unwrap();
    println!("{}", text);
}