use std::collections::HashSet;

use actor::{
    Actor, ActorContext, Error as ActorError, Event, Handler, Message, Response,
};
use async_trait::async_trait;
use types::{Contract, ContractResult, GovernanceData, GovernanceEvent};
use serde::{Deserialize, Serialize};

use crate::{
    governance::{model::SchemaEnum, Member, Policy, Role, Schema, Who}, model::patch::apply_patch, Error, Governance, ValueWrapper
};

pub mod types;

pub struct Runner {}

impl Runner {
    async fn execute_contract(
        state: &ValueWrapper,
        event: &ValueWrapper,
        compiled_contract: Contract,
        is_owner: bool,
    ) -> Result<(), Error> {
        let Contract::CompiledContract(contract_bytes) = compiled_contract else {
            //return Self::execute_governance_contract(state, event).await;
            return Ok(());
        };
        Ok(())
    }

    async fn execute_governance_contract(
        state: &ValueWrapper,
        event: &ValueWrapper,
    ) -> Result<ContractResult, Error> {
        let Ok(event) = serde_json::from_value::<GovernanceEvent>(event.0.clone()) else {
            return Ok(ContractResult::error());
        };
        match &event {
            GovernanceEvent::Patch { data } => {
                let Ok(patched_state) = apply_patch(data.0.clone(), state.0.clone()) else {
                    return Ok(ContractResult::error());
                };
                if Self::check_governance_state(&patched_state).is_ok() {
                    Ok(ContractResult {
                        final_state: ValueWrapper(serde_json::to_value(patched_state).unwrap()),
                        approval_required: true,
                        success: true,
                    })
                } else {
                    Ok(ContractResult {
                        final_state: state.clone(),
                        approval_required: false,
                        success: false,
                    })
                }
            }
        }
    }

    fn check_governance_state(governance: &GovernanceData) -> Result<(), Error> {
        // Nombre e ID unicos
        let (id_set, name_set) = Self::check_members(&governance.members)?;
        // Políticas únicas
        let policies_names = Self::check_policies(&governance.policies)?;
        // Schemas y politicas 1:1, no puede aparecer el schema de governance
        Self::check_schemas(&governance.schemas, policies_names.clone())?;
        // Los roles tiene que ser asignados a miembros, sea por ID o NAME y tienen que pertenecer a una schema (politicas y schemas son las mismas)
        Self::check_roles(&governance.roles, policies_names, id_set, name_set)
    }

    fn check_members(
        members: &[Member],
    ) -> Result<(HashSet<String>, HashSet<String>), Error> {
        let mut name_set = HashSet::new();
        let mut id_set = HashSet::new();

        for member in members {
            if !name_set.insert(member.name.clone()) {
                return Err(Error::Evaluation(
                    "There are duplicate names in members".to_owned(),
                ));
            }
            if !id_set.insert(member.id.clone()) {
                return Err(Error::Evaluation(
                    "There are duplicate id in members".to_owned(),
                ));
            }
        }
        // TODO: Checkear que el nodo es miembro, no se ha eliminado.

        Ok((id_set, name_set))
    }

    fn check_policies(policies: &[Policy]) -> Result<HashSet<String>, Error> {
        let mut is_governance_present = false;
        let mut id_set = HashSet::new();

        for policy in policies {
            if policy.id != "governance" {
                if !id_set.insert(policy.id.clone()) {
                    return Err(Error::Evaluation(
                        "There are duplicate id in polities".to_owned(),
                    ));
                }
            } else if !is_governance_present {
                is_governance_present = true;
            } else {
                return Err(Error::Evaluation(
                    "The policy of governance appears more than once."
                        .to_owned(),
                ));
            }
        }

        if !is_governance_present {
            return Err(Error::Evaluation(
                "governance policy is not present".to_owned(),
            ));
        }

        Ok(id_set)
    }

    fn check_schemas(
        schemas: &[Schema],
        mut policies_names: HashSet<String>,
    ) -> Result<(), Error> {
        // También se tiene que comprobar que los estados iniciales sean válidos según el json_schema, TODO
        if schemas.len() != policies_names.len() {
            return Err(Error::Evaluation(
                "There are a number of different policies and schemes".to_owned(),
            ));
        }

        for schema in schemas {
            if &schema.id == "governance" {
                return Err(Error::Evaluation(
                    "There cannot exist a schema with the name governance".to_owned(),
                ));
            }

            if !policies_names.remove(&schema.id) {
                // Error hay un schema que no está en policies
                return Err(Error::Evaluation(
                    "There is a schema that has no associated policy".to_owned(),
                ));
            }
        }

        if !policies_names.is_empty() {
            // Error hay un policie que no está en schema.
            return Err(Error::Evaluation(
                "There is a policy that has no associated schema".to_owned(),
            ));
        }
        Ok(())
    }

    fn check_roles(
        roles: &[Role],
        mut policies: HashSet<String>,
        id_set: HashSet<String>,
        name_set: HashSet<String>,
    ) -> Result<(), Error> {
        // TODO Checkear que el owner del nodo sigue tiniendo los roles básicos que se han especificado en la meta gov.
        for role in roles {
            if let SchemaEnum::ID { ID } = &role.schema {
                if !policies.contains(ID) {
                    return Err(Error::Evaluation(format!("The role {} of member {} belongs to an invalid schema.", role.role, role.who)));
                }
            }
            match &role.who {
                Who::ID { ID } => {
                    if !id_set.contains(ID) {
                        return Err(Error::Evaluation(format!("No members have been added with this ID: {}", ID)));
                    }
                }
                Who::NAME { NAME } => {
                    if !name_set.contains(NAME) {
                        return Err(Error::Evaluation(format!("No members have been added with this ID: {}", NAME)));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum RunnerCommand {
    Run,
}

impl Message for RunnerCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RunnerEvent {}

impl Event for RunnerEvent {}

#[derive(Debug, Clone)]
pub enum RunnerResponse {
    Error(Error),
    None,
}

impl Response for RunnerResponse {}

#[async_trait]
impl Actor for Runner {
    type Event = RunnerEvent;
    type Message = RunnerCommand;
    type Response = RunnerResponse;
}

#[async_trait]
impl Handler<Runner> for Runner {
    async fn handle_message(
        &mut self,
        msg: RunnerCommand,
        ctx: &mut ActorContext<Runner>,
    ) -> Result<RunnerResponse, ActorError> {
        Ok(RunnerResponse::None)
    }

    async fn on_event(
        &mut self,
        event: RunnerEvent,
        ctx: &mut ActorContext<Runner>,
    ) {
    }
}
