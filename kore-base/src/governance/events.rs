use std::collections::{BTreeSet, HashSet};

use identity::identifier::KeyIdentifier;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{error::Error, model::Namespace};

use super::{
    Governance, Quorum, Role,
    model::{
        CreatorQuantity, RoleCreator, RolesAllSchemas, RolesGov, RolesSchema,
    },
};
pub type MemberName = String;
pub type SchemaId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceEvent {
    pub members: Option<MemberEvent>,
    pub roles: Option<RolesEvent>,
    pub schemas: Option<SchemasEvent>,
    pub policies: Option<PoliciesEvent>,
}

impl GovernanceEvent {
    pub fn is_empty(&self) -> bool {
        self.members.is_none()
            && self.roles.is_none()
            && self.schemas.is_none()
            && self.policies.is_none()
    }
}

///// Members /////
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberEvent {
    pub add: Option<HashSet<NewMember>>,
    pub remove: Option<HashSet<MemberName>>,
    pub change: Option<HashSet<ChangeMember>>,
}

impl MemberEvent {
    pub fn is_empty(&self) -> bool {
        self.add.is_none() && self.remove.is_none() && self.change.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct NewMember {
    pub name: MemberName,
    pub key: KeyIdentifier,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct ChangeMember {
    pub actual_name: MemberName,
    pub new_key: Option<KeyIdentifier>,
    pub new_name: Option<MemberName>,
}

impl ChangeMember {
    pub fn is_empty(&self) -> bool {
        self.actual_name.is_empty()
            || self.new_key.is_none() && self.new_name.is_none()
    }
}

///// Roles /////
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolesEvent {
    pub governance: Option<GovRoleEvent>,
    pub all_schemas: Option<AllSchemasRoleEvent>,
    pub schema: Option<HashSet<SchemaIdRole>>,
}

impl RolesEvent {
    pub fn is_empty(&self) -> bool {
        self.governance.is_none()
            && self.schema.is_none()
            && self.all_schemas.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct GovRoleEvent {
    pub add: Option<GovRolesEvent>,
    pub remove: Option<GovRolesEvent>,
}

impl GovRoleEvent {
    pub fn is_empty(&self) -> bool {
        self.add.is_none() && self.remove.is_none()
    }

    pub fn check_data(
        &self,
        governance: &Governance,
        new_roles: &mut RolesGov,
    ) -> Result<(), Error> {
        if let Some(add) = self.add.clone() {
            if add.is_empty() {
                return Err(Error::Runner(
                    "Add in GovRoleEvent can not be empty".to_owned(),
                ));
            }

            let members: HashSet<String> =
                governance.members.keys().cloned().collect();

            // Approvers
            if let Some(approvers) = add.approver {
                if approvers.is_empty() {
                    return Err(Error::Runner("Approvers vec in governance roles add can not be empty".to_owned()));
                }

                for mut approver in approvers {
                    approver = approver.trim().to_owned();

                    if approver.is_empty() {
                        return Err(Error::Runner(
                            "Approver name in governance roles can not empty"
                                .to_owned(),
                        ));
                    }

                    if approver.len() > 100 {
                        return Err(Error::Runner("Approver name len in governance roles must be less than or equal to 100".to_owned()));
                    }

                    if !members.contains(&approver) {
                        return Err(Error::Runner("Approver name in governance roles is not a governance member".to_owned()));
                    }

                    if !new_roles.approver.insert(approver.clone()) {
                        return Err(Error::Runner(format!(
                            "{} there is already a approver in governance roles",
                            approver
                        )));
                    };
                }
            }

            // Evaluators
            if let Some(evaluators) = add.evaluator {
                if evaluators.is_empty() {
                    return Err(Error::Runner("Evaluators vec in governance roles add can not be empty".to_owned()));
                }

                for mut evaluator in evaluators {
                    evaluator = evaluator.trim().to_owned();

                    if evaluator.is_empty() {
                        return Err(Error::Runner(
                            "Evaluator name in governance roles can not empty"
                                .to_owned(),
                        ));
                    }

                    if evaluator.len() > 100 {
                        return Err(Error::Runner("Evaluator name len in governance roles must be less than or equal to 100".to_owned()));
                    }

                    if !members.contains(&evaluator) {
                        return Err(Error::Runner("Evaluator name in governance roles is not a governance member".to_owned()));
                    }

                    if !new_roles.evaluator.insert(evaluator.clone()) {
                        return Err(Error::Runner(format!(
                            "{} there is already a evaluator in governance roles",
                            evaluator
                        )));
                    };
                }
            }

            // Validators
            if let Some(validators) = add.validator {
                if validators.is_empty() {
                    return Err(Error::Runner("Validators vec in governance roles add can not be empty".to_owned()));
                }

                for mut validator in validators {
                    validator = validator.trim().to_owned();

                    if validator.is_empty() {
                        return Err(Error::Runner(
                            "Validator name in governance roles can not empty"
                                .to_owned(),
                        ));
                    }

                    if validator.len() > 100 {
                        return Err(Error::Runner("Validator name len in governance roles must be less than or equal to 100".to_owned()));
                    }

                    if !members.contains(&validator) {
                        return Err(Error::Runner("Validator name in governance roles is not a governance member".to_owned()));
                    }

                    if !new_roles.validator.insert(validator.clone()) {
                        return Err(Error::Runner(format!(
                            "{} there is already a validator in governance roles",
                            validator
                        )));
                    };
                }
            }

            // Witnesses
            if let Some(witnesses) = add.witness {
                if witnesses.is_empty() {
                    return Err(Error::Runner("Witnesses vec in governance roles add can not be empty".to_owned()));
                }

                for mut witness in witnesses {
                    witness = witness.trim().to_owned();

                    if witness.is_empty() {
                        return Err(Error::Runner(
                            "Witness name in governance roles can not empty"
                                .to_owned(),
                        ));
                    }

                    if witness.len() > 100 {
                        return Err(Error::Runner("Witness name len in governance roles must be less than or equal to 100".to_owned()));
                    }

                    if !members.contains(&witness) {
                        return Err(Error::Runner("Witness name in governance roles is not a governance member".to_owned()));
                    }

                    if !new_roles.witness.insert(witness.clone()) {
                        return Err(Error::Runner(format!(
                            "{} there is already a witness in governance roles",
                            witness
                        )));
                    };
                }
            }

            // Issuers
            if let Some(issuers) = add.issuer {
                if issuers.is_empty() {
                    return Err(Error::Runner(
                        "Issuers vec in governance roles can not be empty"
                            .to_owned(),
                    ));
                }

                for mut issuer in issuers {
                    issuer = issuer.trim().to_owned();

                    if issuer.is_empty() {
                        return Err(Error::Runner(
                            "Issuer name in governance roles can not empty"
                                .to_owned(),
                        ));
                    }

                    if issuer.len() > 100 {
                        return Err(Error::Runner("Issuer name len in governance roles must be less than or equal to 100".to_owned()));
                    }

                    if issuer != "Any" {
                        if !members.contains(&issuer) {
                            return Err(Error::Runner("Issuer name in governance roles is not a governance member".to_owned()));
                        }

                        if !new_roles.issuer.users.insert(issuer.clone()) {
                            return Err(Error::Runner(format!(
                                "{} there is already a issuer in governance roles",
                                issuer
                            )));
                        };
                    } else {
                        new_roles.issuer.any = true;
                    }
                }
            }
        }

        if let Some(remove) = self.remove.clone() {
            if remove.is_empty() {
                return Err(Error::Runner(
                    "Remove in GovRoleEvent can not be empty".to_owned(),
                ));
            }

            // Approvers
            if let Some(approvers) = remove.approver {
                if approvers.is_empty() {
                    return Err(Error::Runner("Approver vec in governance roles remove can not be empty".to_owned()));
                }

                for approver in approvers {
                    if !new_roles.approver.remove(&approver) {
                        return Err(Error::Runner(format!(
                            "Can not remove approver {}, does not have this role",
                            approver
                        )));
                    }
                }
            }

            // Evaluators
            if let Some(evaluators) = remove.evaluator {
                if evaluators.is_empty() {
                    return Err(Error::Runner("Evaluators vec in governance roles remove can not be empty".to_owned()));
                }

                for evaluator in evaluators {
                    if !new_roles.evaluator.remove(&evaluator) {
                        return Err(Error::Runner(format!(
                            "Can not remove evaluator {}, does not have this role",
                            evaluator
                        )));
                    }
                }
            }

            // Evaluators
            if let Some(validators) = remove.validator {
                if validators.is_empty() {
                    return Err(Error::Runner("Validators vec in governance roles remove can not be empty".to_owned()));
                }
                for validator in validators {
                    if !new_roles.validator.remove(&validator) {
                        return Err(Error::Runner(format!(
                            "Can not remove validator {}, does not have this role",
                            validator
                        )));
                    }
                }
            }

            // Witnesses
            if let Some(witnesses) = remove.witness {
                if witnesses.is_empty() {
                    return Err(Error::Runner("Witnesses vec in governance roles remove can not be empty".to_owned()));
                }
                for witness in witnesses {
                    if !new_roles.witness.remove(&witness) {
                        return Err(Error::Runner(format!(
                            "Can not remove witness {}, does not have this role",
                            witness
                        )));
                    };
                }
            }

            // Issuers
            if let Some(issuers) = remove.issuer {
                if issuers.is_empty() {
                    return Err(Error::Runner("Issuers vec in governance roles remove can not be empty".to_owned()));
                }
                for issuer in issuers {
                    if issuer != "Any" {
                        if !new_roles.issuer.users.remove(&issuer) {
                            return Err(Error::Runner(format!(
                                "Can not remove issuer {}, does not have this role",
                                issuer
                            )));
                        };
                    } else {
                        new_roles.issuer.any = false;
                    }
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SchemaIdRole {
    pub schema_id: SchemaId,
    pub roles: SchemaRoleEvent,
}

impl SchemaIdRole {
    pub fn is_empty(&self) -> bool {
        self.schema_id.is_empty()
            || self.roles.add.is_none()
                && self.roles.change.is_none()
                && self.roles.remove.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct AllSchemasRoleEvent {
    pub add: Option<AllSchemasRolesAddEvent>,
    pub remove: Option<AllSchemasRolesRemoveEvent>,
    pub change: Option<AllSchemasRolesChangeEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct SchemaRoleEvent {
    pub add: Option<SchemaRolesAddEvent>,
    pub remove: Option<SchemaRolesRemoveEvent>,
    pub change: Option<SchemaRolesChangeEvent>,
}

impl AllSchemasRoleEvent {
    pub fn check_data(
        &self,
        governance: &Governance,
        roles_not_gov: RolesAllSchemas,
        schema_id: &str,
    ) -> Result<RolesAllSchemas, Error> {
        let schema_role = SchemaRoleEvent::from(self.clone());

        let mut roles_schema = RolesSchema::from(roles_not_gov);
        schema_role.check_data(governance, &mut roles_schema, schema_id)?;
        let roles_not_gov = RolesAllSchemas::from(roles_schema.clone());
        Ok(roles_not_gov)
    }
}

impl From<AllSchemasRoleEvent> for SchemaRoleEvent {
    fn from(value: AllSchemasRoleEvent) -> Self {
        Self {
            add: value.add.map(SchemaRolesAddEvent::from),
            remove: value.remove.map(SchemaRolesRemoveEvent::from),
            change: value.change.map(SchemaRolesChangeEvent::from),
        }
    }
}

impl SchemaRoleEvent {
    pub fn check_data(
        &self,
        governance: &Governance,
        roles_schema: &mut RolesSchema,
        schema_id: &str,
    ) -> Result<(), Error> {
        let members: HashSet<String> =
            governance.members.keys().cloned().collect();

        if let Some(add) = self.add.clone() {
            if add.is_empty() {
                return Err(Error::Runner(
                    "Add in SchemaRolesEvent can not be empty".to_owned(),
                ));
            }

            if let Some(evaluators) = add.evaluator {
                if evaluators.is_empty() {
                    return Err(Error::Runner(
                        "Evaluators vec in schema roles add can not be empty"
                            .to_owned(),
                    ));
                }

                for mut evaluator in evaluators {
                    evaluator.name = evaluator.name.trim().to_owned();

                    if evaluator.name.is_empty() {
                        return Err(Error::Runner(format!(
                            "Evaluator name in schema {} roles can not be empty",
                            schema_id
                        )));
                    }

                    if evaluator.name.len() > 100 {
                        return Err(Error::Runner(format!(
                            "Evaluator name len in schema {} roles must be less than or equal to 100",
                            schema_id
                        )));
                    }

                    if !evaluator.namespace.check() {
                        return Err(Error::Runner(format!(
                            "Evaluator namespace in schema {} roles is invalid",
                            schema_id
                        )));
                    }

                    if !members.contains(&evaluator.name) {
                        return Err(Error::Runner(format!(
                            "Evaluator name in schema {} roles is not a governance member",
                            schema_id
                        )));
                    }

                    if !roles_schema.evaluator.insert(evaluator.clone()) {
                        return Err(Error::Runner(format!(
                            "{} there is already a evaluator in schema {} roles",
                            evaluator.name, schema_id
                        )));
                    };
                }
            }

            if let Some(validators) = add.validator {
                if validators.is_empty() {
                    return Err(Error::Runner(
                        "Validators vec in schema roles add can not be empty"
                            .to_owned(),
                    ));
                }

                for mut validator in validators {
                    validator.name = validator.name.trim().to_owned();

                    if validator.name.is_empty() {
                        return Err(Error::Runner(format!(
                            "Validator name in schema {} roles can not be empty",
                            schema_id
                        )));
                    }

                    if validator.name.len() > 100 {
                        return Err(Error::Runner(format!(
                            "Validator name len in schema {} roles must be less than or equal to 100",
                            schema_id
                        )));
                    }

                    if !validator.namespace.check() {
                        return Err(Error::Runner(format!(
                            "Validator namespace in schema {} roles is invalid",
                            schema_id
                        )));
                    }

                    if !members.contains(&validator.name) {
                        return Err(Error::Runner(format!(
                            "Validator name in schema {} roles is not a governance member",
                            schema_id
                        )));
                    }

                    if !roles_schema.validator.insert(validator.clone()) {
                        return Err(Error::Runner(format!(
                            "{} there is already a validator in schema {} roles",
                            validator.name, schema_id
                        )));
                    };
                }
            }

            if let Some(witnesses) = add.witness {
                if witnesses.is_empty() {
                    return Err(Error::Runner(
                        "Witnesses vec in schema roles add can not be empty"
                            .to_owned(),
                    ));
                }

                for mut witness in witnesses {
                    witness.name = witness.name.trim().to_owned();

                    if witness.name.is_empty() {
                        return Err(Error::Runner(format!(
                            "Witness name in schema {} roles can not be empty",
                            schema_id
                        )));
                    }

                    if witness.name.len() > 100 {
                        return Err(Error::Runner(format!(
                            "Witness name len in schema {} roles must be less than or equal to 100",
                            schema_id
                        )));
                    }

                    if !witness.namespace.check() {
                        return Err(Error::Runner(format!(
                            "Witness namespace in schema {} roles is invalid",
                            schema_id
                        )));
                    }

                    if !members.contains(&witness.name) {
                        return Err(Error::Runner(format!(
                            "Witness name in schema {} roles is not a governance member",
                            schema_id
                        )));
                    }

                    if !roles_schema.witness.insert(witness.clone()) {
                        return Err(Error::Runner(format!(
                            "{} there is already a witness in schema {} roles",
                            witness.name, schema_id
                        )));
                    };
                }
            }

            if let Some(creators) = add.creator {
                if creators.is_empty() {
                    return Err(Error::Runner(
                        "Creators vec in schema roles add can not be empty"
                            .to_owned(),
                    ));
                }

                for mut creator in creators {
                    creator.name = creator.name.trim().to_owned();

                    if creator.name.is_empty() {
                        return Err(Error::Runner(format!(
                            "Creator name in schema {} roles can not be empty",
                            schema_id
                        )));
                    }

                    if creator.name.len() > 100 {
                        return Err(Error::Runner(format!(
                            "Creator name len in schema {} roles must be less than or equal to 100",
                            schema_id
                        )));
                    }

                    if !creator.quantity.check() {
                        return Err(Error::Runner(
                            "Creator quantity in schema roles can not be 0"
                                .to_owned(),
                        ));
                    }

                    if !creator.namespace.check() {
                        return Err(Error::Runner(format!(
                            "Creator namespace in schema {} roles is invalid",
                            schema_id
                        )));
                    }

                    if !members.contains(&creator.name) {
                        return Err(Error::Runner(format!(
                            "Creator name in schema {} roles is not a governance member",
                            schema_id
                        )));
                    }

                    for witness in creator.witnesses.iter() {
                        if witness != "Witnesses" && !members.contains(witness)
                        {
                            return Err(Error::Runner(format!(
                                "Witness of Creator in schema {} roles is not a governance member",
                                schema_id
                            )));
                        }
                    }

                    if !roles_schema.creator.insert(creator.clone()) {
                        return Err(Error::Runner(format!(
                            "{} there is already a creator in schema {} roles",
                            creator.name, schema_id
                        )));
                    };
                }
            }

            if let Some(issuers) = add.issuer {
                if issuers.is_empty() {
                    return Err(Error::Runner(
                        "Issuers vec in schema roles add can not be empty"
                            .to_owned(),
                    ));
                }

                for mut issuer in issuers {
                    issuer.name = issuer.name.trim().to_owned();

                    if issuer.name.is_empty() {
                        return Err(Error::Runner(format!(
                            "Issuer name in schema {} roles can not be empty",
                            schema_id
                        )));
                    }

                    if issuer.name.len() > 100 {
                        return Err(Error::Runner(format!(
                            "Issuer name len in schema {} roles must be less than or equal to 100",
                            schema_id
                        )));
                    }

                    if issuer.name != "Any" {
                        if !issuer.namespace.check() {
                            return Err(Error::Runner(format!(
                                "Issuer namespace in schema {} roles is invalid",
                                schema_id
                            )));
                        }

                        if !members.contains(&issuer.name) {
                            return Err(Error::Runner(format!(
                                "Issuer name in schema {} roles is not a governance member",
                                schema_id
                            )));
                        }

                        if !roles_schema.issuer.users.insert(issuer.clone()) {
                            return Err(Error::Runner(format!(
                                "{} there is already a issuer in schema {} roles",
                                issuer.name, schema_id
                            )));
                        };
                    } else {
                        if !issuer.namespace.is_empty() {
                            return Err(Error::Runner(format!(
                                "Can not add issuer Any in schema {}, Namespace must be empty",
                                schema_id
                            )));
                        }

                        roles_schema.issuer.any = true;
                    }
                }
            }
        }

        if let Some(remove) = self.remove.clone() {
            if remove.is_empty() {
                return Err(Error::Runner(
                    "Remove in SchemaRolesEvent can not be empty".to_owned(),
                ));
            }

            if let Some(evaluators) = remove.evaluator {
                if evaluators.is_empty() {
                    return Err(Error::Runner("Evaluators vec in schema roles remove can not be empty".to_owned()));
                }

                for evaluator in evaluators {
                    if !roles_schema.evaluator.remove(&evaluator) {
                        return Err(Error::Runner(format!(
                            "Can not remove evaluator {} {} from {} schema, does not have this role",
                            evaluator.name, evaluator.namespace, schema_id
                        )));
                    };
                }
            }

            if let Some(validators) = remove.validator {
                if validators.is_empty() {
                    return Err(Error::Runner("Validators vec in schema roles remove can not be empty".to_owned()));
                }

                for validator in validators {
                    if !roles_schema.validator.remove(&validator) {
                        return Err(Error::Runner(format!(
                            "Can not remove validator {} {} from {} schema, does not have this role",
                            validator.name, validator.namespace, schema_id
                        )));
                    };
                }
            }

            if let Some(witnesses) = remove.witness {
                if witnesses.is_empty() {
                    return Err(Error::Runner(
                        "Witnesses vec in schema roles remove can not be empty"
                            .to_owned(),
                    ));
                }

                for witness in witnesses {
                    if !roles_schema.witness.remove(&witness) {
                        return Err(Error::Runner(format!(
                            "Can not remove witness {} {} from {} schema, does not have this role",
                            witness.name, witness.namespace, schema_id
                        )));
                    };
                }
            }

            if let Some(creators) = remove.creator {
                if creators.is_empty() {
                    return Err(Error::Runner(
                        "Creators vec in schema roles remove can not be empty"
                            .to_owned(),
                    ));
                }

                for creator in creators {
                    if !roles_schema.creator.remove(&RoleCreator::create(
                        &creator.name,
                        creator.namespace.clone(),
                    )) {
                        return Err(Error::Runner(format!(
                            "Can not remove creator {} {} from {} schema, does not have this role",
                            creator.name, creator.namespace, schema_id
                        )));
                    }
                }
            }

            if let Some(issuers) = remove.issuer {
                if issuers.is_empty() {
                    return Err(Error::Runner(
                        "Issuers vec in schema roles remove can not be empty"
                            .to_owned(),
                    ));
                }

                for issuer in issuers {
                    if issuer.name != "Any" {
                        if !roles_schema.issuer.users.remove(&issuer) {
                            return Err(Error::Runner(format!(
                                "Can not remove issuer {} {} from {} schema, does not have this role",
                                issuer.name, issuer.namespace, schema_id
                            )));
                        }
                    } else {
                        if !issuer.namespace.is_empty() {
                            return Err(Error::Runner("Can not remove issuer Any, Namespace must be empty".to_owned()));
                        }
                        roles_schema.issuer.any = false;
                    }
                }
            }
        }

        if let Some(change) = self.change.clone() {
            if change.is_empty() {
                return Err(Error::Runner(
                    "Change in SchemaRolesEvent can not be empty".to_owned(),
                ));
            }

            if let Some(evaluators) = change.evaluator {
                if evaluators.is_empty() {
                    return Err(Error::Runner("Evaluators vec in schema roles change can not be empty".to_owned()));
                }

                for evaluator in evaluators {
                    if !evaluator.new_namespace.check() {
                        return Err(Error::Runner(format!(
                            "Can not change evaluator {} {} from {} schema, invalid new namespace",
                            evaluator.actual_name,
                            evaluator.actual_namespace,
                            schema_id
                        )));
                    }

                    if !roles_schema.evaluator.remove(&Role {
                        name: evaluator.actual_name.clone(),
                        namespace: evaluator.actual_namespace.clone(),
                    }) {
                        return Err(Error::Runner(format!(
                            "Can not change evaluator {} {} from {} schema, does not have this role",
                            evaluator.actual_name,
                            evaluator.actual_namespace,
                            schema_id
                        )));
                    };

                    if !roles_schema.evaluator.insert(Role {
                        name: evaluator.actual_name.clone(),
                        namespace: evaluator.new_namespace.clone(),
                    }) {
                        return Err(Error::Runner(format!(
                            "Can not change evaluator {} {} from {} schema, evaluator whith this namespace already exist",
                            evaluator.actual_name,
                            evaluator.actual_namespace,
                            schema_id
                        )));
                    }
                }
            }

            if let Some(validators) = change.validator {
                if validators.is_empty() {
                    return Err(Error::Runner("Validators vec in schema roles change can not be empty".to_owned()));
                }

                for validator in validators {
                    if !validator.new_namespace.check() {
                        return Err(Error::Runner(format!(
                            "Can not change validator {} {} from {} schema, invalid new namespace",
                            validator.actual_name,
                            validator.actual_namespace,
                            schema_id
                        )));
                    }

                    if !roles_schema.validator.remove(&Role {
                        name: validator.actual_name.clone(),
                        namespace: validator.actual_namespace.clone(),
                    }) {
                        return Err(Error::Runner(format!(
                            "Can not change validator {} {} from {} schema, does not have this role",
                            validator.actual_name,
                            validator.actual_namespace,
                            schema_id
                        )));
                    };

                    if !roles_schema.validator.insert(Role {
                        name: validator.actual_name.clone(),
                        namespace: validator.new_namespace.clone(),
                    }) {
                        return Err(Error::Runner(format!(
                            "Can not change validator {} {} from {} schema, validator whith this namespace already exist",
                            validator.actual_name,
                            validator.actual_namespace,
                            schema_id
                        )));
                    }
                }
            }

            if let Some(witnesses) = change.witness {
                if witnesses.is_empty() {
                    return Err(Error::Runner(
                        "Witnesses vec in schema roles change can not be empty"
                            .to_owned(),
                    ));
                }

                for witness in witnesses {
                    if !witness.new_namespace.check() {
                        return Err(Error::Runner(format!(
                            "Can not change witness {} {} from {} schema, invalid new namespace",
                            witness.actual_name,
                            witness.actual_namespace,
                            schema_id
                        )));
                    }

                    if !roles_schema.witness.remove(&Role {
                        name: witness.actual_name.clone(),
                        namespace: witness.actual_namespace.clone(),
                    }) {
                        return Err(Error::Runner(format!(
                            "Can not change witness {} {} from {} schema, does not have this role",
                            witness.actual_name,
                            witness.actual_namespace,
                            schema_id
                        )));
                    };

                    if !roles_schema.witness.insert(Role {
                        name: witness.actual_name.clone(),
                        namespace: witness.new_namespace.clone(),
                    }) {
                        return Err(Error::Runner(format!(
                            "Can not change witness {} {} from {} schema, witness whith this namespace already exist",
                            witness.actual_name,
                            witness.actual_namespace,
                            schema_id
                        )));
                    }
                }
            }

            if let Some(creators) = change.creator {
                if creators.is_empty() {
                    return Err(Error::Runner(
                        "Creators vec in schema roles change can not be empty"
                            .to_owned(),
                    ));
                }

                for creator in creators {
                    if creator.is_empty() {
                        return Err(Error::Runner(format!(
                            "Can not change creator {} {} from {} schema, has not new namespace or new quantity or new witnesses",
                            creator.actual_name,
                            creator.actual_namespace,
                            schema_id
                        )));
                    }

                    let Some(old_creator) =
                        roles_schema.creator.take(&RoleCreator::create(
                            &creator.actual_name,
                            creator.actual_namespace.clone(),
                        ))
                    else {
                        return Err(Error::Runner(format!(
                            "Can not change creator {} {} from {} schema, does not have this role",
                            creator.actual_name,
                            creator.actual_namespace,
                            schema_id
                        )));
                    };

                    let new_namespace = if let Some(new_namespace) =
                        creator.new_namespace
                    {
                        if !new_namespace.check() {
                            return Err(Error::Runner(format!(
                                "Can not change creator {} {} from {} schema, invalid new namespace",
                                creator.actual_name,
                                creator.actual_namespace,
                                schema_id
                            )));
                        }
                        new_namespace
                    } else {
                        old_creator.namespace
                    };

                    let new_quantity =
                        if let Some(quantity) = creator.new_quantity {
                            if !quantity.check() {
                                return Err(Error::Runner(
                                "Creator quantity in schema roles can not be 0"
                                    .to_owned(),
                            ));
                            }
                            quantity
                        } else {
                            old_creator.quantity
                        };

                    let new_witnesses = if let Some(witnesses) =
                        creator.new_witnesses
                    {
                        let mut witnesses = witnesses.clone();

                        if witnesses.is_empty() {
                            witnesses.insert("Witnesses".to_owned());
                        }

                        for witness in witnesses.iter() {
                            if witness != "Witnesses"
                                && !members.contains(witness)
                            {
                                return Err(Error::Runner(format!(
                                    "Witness of Creator in schema {} roles is not a governance member",
                                    schema_id
                                )));
                            }
                        }

                        witnesses
                    } else {
                        old_creator.witnesses
                    };

                    if !roles_schema.creator.insert(RoleCreator {
                        name: creator.actual_name.clone(),
                        namespace: new_namespace,
                        quantity: new_quantity,
                        witnesses: new_witnesses,
                    }) {
                        return Err(Error::Runner(format!(
                            "Can not change creator {} {} from {} schema, creator whith this namespace already exist",
                            creator.actual_name,
                            creator.actual_namespace,
                            schema_id
                        )));
                    }
                }
            }

            if let Some(issuers) = change.issuer {
                if issuers.is_empty() {
                    return Err(Error::Runner(
                        "Issuers vec in schema roles change can not be empty"
                            .to_owned(),
                    ));
                }

                for issuer in issuers {
                    if issuer.actual_name != "Any" {
                        if !issuer.new_namespace.check() {
                            return Err(Error::Runner(format!(
                                "Can not change issuer {} {} from {} schema, invalid new namespace",
                                issuer.actual_name,
                                issuer.actual_namespace,
                                schema_id
                            )));
                        }

                        if !roles_schema.issuer.users.remove(&Role {
                            name: issuer.actual_name.clone(),
                            namespace: issuer.actual_namespace.clone(),
                        }) {
                            return Err(Error::Runner(format!(
                                "Can not change issuer {} {} from {} schema, does not have this role",
                                issuer.actual_name,
                                issuer.actual_namespace,
                                schema_id
                            )));
                        };

                        if !roles_schema.issuer.users.insert(Role {
                            name: issuer.actual_name.clone(),
                            namespace: issuer.new_namespace.clone(),
                        }) {
                            return Err(Error::Runner(format!(
                                "Can not change issuer {} {} from {} schema, issuer whith this namespace already exist",
                                issuer.actual_name,
                                issuer.actual_namespace,
                                schema_id
                            )));
                        }
                    } else {
                        return Err(Error::Runner(
                            "Can not change issuer Any".to_owned(),
                        ));
                    }
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct GovRolesEvent {
    pub approver: Option<BTreeSet<MemberName>>,
    pub evaluator: Option<BTreeSet<MemberName>>,
    pub validator: Option<BTreeSet<MemberName>>,
    pub witness: Option<BTreeSet<MemberName>>,
    pub issuer: Option<BTreeSet<MemberName>>,
}

impl GovRolesEvent {
    pub fn is_empty(&self) -> bool {
        self.approver.is_none()
            && self.evaluator.is_none()
            && self.validator.is_none()
            && self.witness.is_none()
            && self.issuer.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AllSchemasRolesAddEvent {
    pub evaluator: Option<BTreeSet<Role>>,
    pub validator: Option<BTreeSet<Role>>,
    pub witness: Option<BTreeSet<Role>>,
    pub issuer: Option<BTreeSet<Role>>,
}

impl AllSchemasRolesAddEvent {
    pub fn is_empty(&self) -> bool {
        self.evaluator.is_none()
            && self.validator.is_none()
            && self.witness.is_none()
            && self.issuer.is_none()
    }
}

impl From<AllSchemasRolesAddEvent> for SchemaRolesAddEvent {
    fn from(value: AllSchemasRolesAddEvent) -> Self {
        Self {
            evaluator: value.evaluator,
            validator: value.validator,
            witness: value.witness,
            creator: None,
            issuer: value.issuer,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SchemaRolesAddEvent {
    pub evaluator: Option<BTreeSet<Role>>,
    pub validator: Option<BTreeSet<Role>>,
    pub witness: Option<BTreeSet<Role>>,
    pub creator: Option<BTreeSet<RoleCreator>>,
    pub issuer: Option<BTreeSet<Role>>,
}

impl SchemaRolesAddEvent {
    pub fn is_empty(&self) -> bool {
        self.creator.is_none()
            && self.evaluator.is_none()
            && self.validator.is_none()
            && self.witness.is_none()
            && self.issuer.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AllSchemasRolesRemoveEvent {
    pub evaluator: Option<BTreeSet<Role>>,
    pub validator: Option<BTreeSet<Role>>,
    pub witness: Option<BTreeSet<Role>>,
    pub issuer: Option<BTreeSet<Role>>,
}

impl AllSchemasRolesRemoveEvent {
    pub fn is_empty(&self) -> bool {
        self.evaluator.is_none()
            && self.validator.is_none()
            && self.witness.is_none()
            && self.issuer.is_none()
    }
}

impl From<AllSchemasRolesRemoveEvent> for SchemaRolesRemoveEvent {
    fn from(value: AllSchemasRolesRemoveEvent) -> Self {
        Self {
            evaluator: value.evaluator,
            validator: value.validator,
            witness: value.witness,
            creator: None,
            issuer: value.issuer,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SchemaRolesRemoveEvent {
    pub evaluator: Option<BTreeSet<Role>>,
    pub validator: Option<BTreeSet<Role>>,
    pub witness: Option<BTreeSet<Role>>,
    pub creator: Option<BTreeSet<Role>>,
    pub issuer: Option<BTreeSet<Role>>,
}

impl SchemaRolesRemoveEvent {
    pub fn is_empty(&self) -> bool {
        self.creator.is_none()
            && self.evaluator.is_none()
            && self.validator.is_none()
            && self.witness.is_none()
            && self.issuer.is_none()
    }
}

#[derive(
    Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct AllSchemasRolesChangeEvent {
    pub evaluator: Option<BTreeSet<RoleChange>>,
    pub validator: Option<BTreeSet<RoleChange>>,
    pub witness: Option<BTreeSet<RoleChange>>,
    pub issuer: Option<BTreeSet<RoleChange>>,
}

impl AllSchemasRolesChangeEvent {
    pub fn is_empty(&self) -> bool {
        self.evaluator.is_none()
            && self.validator.is_none()
            && self.witness.is_none()
            && self.issuer.is_none()
    }
}

impl From<AllSchemasRolesChangeEvent> for SchemaRolesChangeEvent {
    fn from(value: AllSchemasRolesChangeEvent) -> Self {
        Self {
            evaluator: value.evaluator,
            validator: value.validator,
            witness: value.witness,
            creator: None,
            issuer: value.issuer,
        }
    }
}

#[derive(
    Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct SchemaRolesChangeEvent {
    pub evaluator: Option<BTreeSet<RoleChange>>,
    pub validator: Option<BTreeSet<RoleChange>>,
    pub witness: Option<BTreeSet<RoleChange>>,
    pub creator: Option<BTreeSet<RoleCreatorChange>>,
    pub issuer: Option<BTreeSet<RoleChange>>,
}

impl SchemaRolesChangeEvent {
    pub fn is_empty(&self) -> bool {
        self.creator.is_none()
            && self.evaluator.is_none()
            && self.validator.is_none()
            && self.witness.is_none()
            && self.issuer.is_none()
    }
}

#[derive(
    Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct RoleCreatorChange {
    pub actual_name: MemberName,
    pub actual_namespace: Namespace,
    pub new_namespace: Option<Namespace>,
    pub new_witnesses: Option<BTreeSet<String>>,
    pub new_quantity: Option<CreatorQuantity>,
}

impl RoleCreatorChange {
    pub fn is_empty(&self) -> bool {
        self.new_namespace.is_none()
            && self.new_quantity.is_none()
            && self.new_witnesses.is_none()
    }
}

#[derive(
    Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct RoleChange {
    pub actual_name: MemberName,
    pub actual_namespace: Namespace,
    pub new_namespace: Namespace,
}

///// Schemas /////
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemasEvent {
    pub add: Option<HashSet<SchemaAdd>>,
    pub remove: Option<HashSet<SchemaId>>,
    pub change: Option<HashSet<SchemaChange>>,
}

impl SchemasEvent {
    pub fn is_empty(&self) -> bool {
        self.add.is_none() && self.remove.is_none() && self.change.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct SchemaAdd {
    pub id: SchemaId,
    pub contract: String,
    pub initial_value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct SchemaChange {
    pub actual_id: SchemaId,
    pub new_contract: Option<String>,
    pub new_initial_value: Option<Value>,
}

impl SchemaChange {
    pub fn is_empty(&self) -> bool {
        self.actual_id.is_empty()
            || self.new_contract.is_none() && self.new_initial_value.is_none()
    }
}

///// Policies /////
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoliciesEvent {
    pub governance: Option<GovPolicieEvent>,
    pub schema: Option<HashSet<SchemaIdPolicie>>,
}

impl PoliciesEvent {
    pub fn is_empty(&self) -> bool {
        self.governance.is_none() && self.schema.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct SchemaIdPolicie {
    pub schema_id: SchemaId,
    pub policies: SchemaPolicieEvent,
}

impl SchemaIdPolicie {
    pub fn is_empty(&self) -> bool {
        self.schema_id.is_empty() || self.policies.change.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct SchemaPolicieEvent {
    pub change: Option<SchemaPolicieChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovPolicieEvent {
    pub change: GovPolicieChange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovPolicieChange {
    pub approve: Option<Quorum>,
    pub evaluate: Option<Quorum>,
    pub validate: Option<Quorum>,
}

impl GovPolicieChange {
    pub fn is_empty(&self) -> bool {
        self.approve.is_none()
            && self.evaluate.is_none()
            && self.validate.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct SchemaPolicieChange {
    pub evaluate: Option<Quorum>,
    pub validate: Option<Quorum>,
}

impl SchemaPolicieChange {
    pub fn is_empty(&self) -> bool {
        self.evaluate.is_none() && self.validate.is_none()
    }
}
