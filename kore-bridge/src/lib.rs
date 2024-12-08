use std::{future::Future, str::FromStr};

use config::Config;
use identity::identifier::{DigestIdentifier, KeyIdentifier};
use kore_base::{ approval::approver::ApprovalStateRes, auth::AuthWitness, error::Error, helpers::db::{EventDB, Paginator, SignaturesDB, SubjectDB}, model::{request::EventRequest, signature::{Signature, Signed}}, request::RequestData, Api as KoreApi };
use model::BridgeSignedEventRequest;
use prometheus_client::registry::Registry;
use tokio_util::sync::CancellationToken;
use utils::key_pair;

mod model;
mod config;
mod settings;
mod utils;

pub struct Bridge {
    api: KoreApi,
    cancellation: CancellationToken
}

impl Bridge {
    pub async fn build(settings: Config, password: &str, token: Option<CancellationToken>,  shutdown_signal: Option<impl Future + Send + 'static>) -> Result<Self, Error> {
        let keys = key_pair(&settings, password)?;
        let mut registry = <Registry>::default();

        let token = if let Some(token) = token {
            token } else {
            CancellationToken::new()
        };

        let api = KoreApi::new(keys, settings.kore_config, &mut registry, password, &token).await?;

        if let Some (shutdown_signal) = shutdown_signal {
            Self::bind_with_shutdown(token.clone(), shutdown_signal);
        } else {
            Self::bind_with_shutdown(token.clone(), tokio::signal::ctrl_c());
        };

        Ok(Self {
            api,
            cancellation: token,
        })
    }

    pub fn token(&self) -> &CancellationToken {
        &self.cancellation
    }

    fn bind_with_shutdown(token: CancellationToken, shutdown_signal: impl Future + Send + 'static) {
        let cancellation_token = token.clone();
        tokio::spawn(async move {
            shutdown_signal.await;
            cancellation_token.cancel();
        });
    }

    pub fn peer_id(&self) -> String {
        self.api.peer_id()
    }

    pub fn controller_id(&self) -> String {
        self.api.controller_id()
    }

    pub async fn send_event_request(
        &self,
        request: BridgeSignedEventRequest
    ) -> Result<RequestData, Error> {
        let event: EventRequest = EventRequest::try_from(request.request)?;
        if let Some(signature) = request.signature {
            let signature = Signature::try_from(signature)?;

            let signed_request = Signed {
                content: event,
                signature,
            };

            self.api.external_request(signed_request).await
        } else {
            self.api.own_request(event).await
        }
    }

    pub async fn get_request_state(
        &self,
        request_id: String
    ) -> Result<String, Error> {
        let request_id = DigestIdentifier::from_str(&request_id).map_err(|e| Error::Bridge(format!("Invalid request id: {}", e)))?;
        self.api.request_state(request_id).await
    }

    pub async fn get_approval(
        &self,
        subject_id: String,
    ) -> Result<(String, String), Error> {
        let subject_id = DigestIdentifier::from_str(&subject_id).map_err(|e| Error::Bridge(format!("Invalid subject id: {}", e)))?;
        self.api.get_approval(subject_id).await
    }

    pub async fn post_approve(
        &self,
        subject_id: String,
        state: String
    ) -> Result<String, Error> {
        let state = match state.as_str() {
            "Accepted" => ApprovalStateRes::RespondedAccepted, 
            "Rejected" => ApprovalStateRes::RespondedRejected,
            _ => return Err(Error::Bridge("Invalid approve response".to_owned()))
        };

        let subject_id = DigestIdentifier::from_str(&subject_id).map_err(|e| Error::Bridge(format!("Invalid subject id: {}", e)))?;

        self.api.approve(subject_id, state).await
    }

    pub async fn put_auth_subject(
        &self,
        subject_id: String,
        witnesses: Vec<String>
    ) -> Result<String, Error> {
        let subject_id = DigestIdentifier::from_str(&subject_id).map_err(|e| Error::Bridge(format!("Invalid subject id: {}", e)))?;
        let mut witnesses_key = vec![];

        for witness in witnesses {
            witnesses_key.push(KeyIdentifier::from_str(&witness).map_err(|e| Error::Bridge(format!("Invalid key identifier: {}", e)))?);
        }

        let auh_witness = if witnesses_key.is_empty() {
            AuthWitness::None
        } else if witnesses_key.len() == 1 {
                AuthWitness::One(witnesses_key[0].clone())
            
        } else {
            AuthWitness::Many(witnesses_key)
        };

        self.api.auth_subject(subject_id, auh_witness).await
    }

    pub async fn get_all_auth_subjects(&self) -> Result<Vec<String>, Error> {
        self.api.all_auth_subjects().await
    }

    pub async fn get_witnesses_subject(
        &self,
        subject_id: String,
    ) -> Result<Vec<String>, Error> {
        let subject_id = DigestIdentifier::from_str(&subject_id).map_err(|e| Error::Bridge(format!("Invalid subject id: {}", e)))?;
        let auth_witness = self.api.witnesses_subject(subject_id).await?;

        match auth_witness {
            AuthWitness::One(key_identifier) => Ok(vec![key_identifier.to_string()]),
            AuthWitness::Many(vec) => Ok(vec.iter().map(|x| x.to_string()).collect()),
            AuthWitness::None => Ok(vec![]),
        }
    }

    pub async fn delete_auth_subject(
        &self,
        subject_id: String,
    ) -> Result<String, Error> {
        let subject_id = DigestIdentifier::from_str(&subject_id).map_err(|e| Error::Bridge(format!("Invalid subject id: {}", e)))?;
        self.api.delete_auth_subject(subject_id).await
    }

    pub async fn update_subject(
        &self,
        subject_id: String,
    ) -> Result<String, Error> {
        let subject_id = DigestIdentifier::from_str(&subject_id).map_err(|e| Error::Bridge(format!("Invalid subject id: {}", e)))?;
        self.api.update_subject(subject_id).await
    }

    pub async fn get_all_govs(&self) -> Result<Vec<String>, Error> {
        self.api.all_govs().await
    }

    pub async fn get_all_subjs(
        &self,
        gov_id: String,
        active: Option<bool>,
        schema: Option<String>,
    ) -> Result<Vec<String>, Error> {
        let gov_id = DigestIdentifier::from_str(&gov_id).map_err(|e| Error::Bridge(format!("Invalid governance id: {}", e)))?;
        self.api.all_subjs(gov_id, active, schema).await
    }

    pub async fn get_events(
        &self,
        subject_id: String,
        quantity: Option<u64>,
        page: Option<u64>,
    ) -> Result<(Vec<EventDB>, Paginator), Error> {
        let subject_id = DigestIdentifier::from_str(&subject_id).map_err(|e| Error::Bridge(format!("Invalid subject id: {}", e)))?;
        self.api.get_events(subject_id, quantity, page).await
    }

    pub async fn get_subject(
        &self,
        subject_id: String,
    ) -> Result<SubjectDB, Error> {
        let subject_id = DigestIdentifier::from_str(&subject_id).map_err(|e| Error::Bridge(format!("Invalid subject id: {}", e)))?;
        self.api.get_subject(subject_id).await
    }

    pub async fn get_signatures(
        &self,
        subject_id: String,
    ) -> Result<SignaturesDB, Error> {
        let subject_id = DigestIdentifier::from_str(&subject_id).map_err(|e| Error::Bridge(format!("Invalid subject id: {}", e)))?;
        self.api.get_signatures(subject_id).await
    }
}