//

use thiserror::Error;

/// Errors related to the Kore Client.
#[derive(Error, Debug)]
pub enum Error {
    /// Initialization error.
    #[error("Initialization error: {0}")]
    Init(String),
    /// Network related error.
    #[error("Network error: {0}")]
    Network(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_error_creation() {
        let error = Error::Init("Failed to initialize".to_string());
        
        match error {
            Error::Init(msg) => assert_eq!(msg, "Failed to initialize"),
            _ => panic!("Expected Init error variant"),
        }
    }

    #[test]
    fn test_network_error_creation() {
        let error = Error::Network("Connection timeout".to_string());
        
        match error {
            Error::Network(msg) => assert_eq!(msg, "Connection timeout"),
            _ => panic!("Expected Network error variant"),
        }
    }

    #[test]
    fn test_init_error_display() {
        let error = Error::Init("Database connection failed".to_string());
        let error_string = format!("{}", error);
        
        assert_eq!(error_string, "Initialization error: Database connection failed");
    }

    #[test]
    fn test_network_error_display() {
        let error = Error::Network("Peer unreachable".to_string());
        let error_string = format!("{}", error);
        
        assert_eq!(error_string, "Network error: Peer unreachable");
    }

    #[test]
    fn test_init_error_debug() {
        let error = Error::Init("Config parse error".to_string());
        let debug_string = format!("{:?}", error);
        
        assert!(debug_string.contains("Init"));
        assert!(debug_string.contains("Config parse error"));
    }

    #[test]
    fn test_network_error_debug() {
        let error = Error::Network("Message serialization failed".to_string());
        let debug_string = format!("{:?}", error);
        
        assert!(debug_string.contains("Network"));
        assert!(debug_string.contains("Message serialization failed"));
    }

    #[test]
    fn test_error_with_empty_message() {
        let init_error = Error::Init(String::new());
        assert_eq!(format!("{}", init_error), "Initialization error: ");
        
        let network_error = Error::Network(String::new());
        assert_eq!(format!("{}", network_error), "Network error: ");
    }

    #[test]
    fn test_error_with_special_characters() {
        let error = Error::Init("Error: \"invalid\" config\n\ttab".to_string());
        let error_string = format!("{}", error);
        
        assert!(error_string.contains("\"invalid\""));
        assert!(error_string.contains("\n\ttab"));
    }

    #[test]
    fn test_error_source() {
        let init_error = Error::Init("test error".to_string());
        // Error type implements std::error::Error, so source() should return None
        assert!(std::error::Error::source(&init_error).is_none());
        
        let network_error = Error::Network("test error".to_string());
        assert!(std::error::Error::source(&network_error).is_none());
    }

    #[test]
    fn test_multiple_errors_different_types() {
        let errors: Vec<Error> = vec![
            Error::Init("init 1".to_string()),
            Error::Network("network 1".to_string()),
            Error::Init("init 2".to_string()),
            Error::Network("network 2".to_string()),
        ];
        
        assert_eq!(errors.len(), 4);
        
        match &errors[0] {
            Error::Init(_) => {},
            _ => panic!("Expected Init error"),
        }
        
        match &errors[1] {
            Error::Network(_) => {},
            _ => panic!("Expected Network error"),
        }
    }

    #[test]
    fn test_error_pattern_matching() {
        fn handle_error(error: Error) -> String {
            match error {
                Error::Init(msg) => format!("Initialization failed: {}", msg),
                Error::Network(msg) => format!("Network issue: {}", msg),
            }
        }
        
        let init_result = handle_error(Error::Init("startup".to_string()));
        assert_eq!(init_result, "Initialization failed: startup");
        
        let network_result = handle_error(Error::Network("timeout".to_string()));
        assert_eq!(network_result, "Network issue: timeout");
    }

    #[test]
    fn test_error_in_result_type() {
        fn returns_init_error() -> Result<(), Error> {
            Err(Error::Init("test".to_string()))
        }
        
        fn returns_network_error() -> Result<(), Error> {
            Err(Error::Network("test".to_string()))
        }
        
        assert!(returns_init_error().is_err());
        assert!(returns_network_error().is_err());
        
        match returns_init_error() {
            Err(Error::Init(_)) => {},
            _ => panic!("Expected Init error"),
        }
        
        match returns_network_error() {
            Err(Error::Network(_)) => {},
            _ => panic!("Expected Network error"),
        }
    }

    #[test]
    fn test_error_conversion_to_string() {
        let init_error = Error::Init("conversion test".to_string());
        let init_string = init_error.to_string();
        assert_eq!(init_string, "Initialization error: conversion test");
        
        let network_error = Error::Network("conversion test".to_string());
        let network_string = network_error.to_string();
        assert_eq!(network_string, "Network error: conversion test");
    }

    #[test]
    fn test_long_error_messages() {
        let long_message = "a".repeat(1000);
        let error = Error::Init(long_message.clone());
        
        match error {
            Error::Init(msg) => assert_eq!(msg.len(), 1000),
            _ => panic!("Expected Init error"),
        }
        
        let display = format!("{}", Error::Network(long_message.clone()));
        assert!(display.len() > 1000);
    }

    #[test]
    fn test_error_equality_not_implemented() {
        // Error doesn't derive PartialEq, so this test just ensures
        // we can create and use errors independently
        let error1 = Error::Init("test".to_string());
        let error2 = Error::Init("test".to_string());
        
        // We can't compare them directly, but we can match on them
        let msg1 = match error1 {
            Error::Init(m) => m,
            Error::Network(m) => m,
        };
        
        let msg2 = match error2 {
            Error::Init(m) => m,
            Error::Network(m) => m,
        };
        
        assert_eq!(msg1, msg2);
    }
}
