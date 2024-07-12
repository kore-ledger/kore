// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    db::Database,
    model::{
        request::{EventRequest, StartRequest},
        signature::Signed,
    },
    Error,
};

use actor::{Actor, ActorContext, Event, Handler, Message, Response};
use async_trait::async_trait;
use identity::identifier::DigestIdentifier;
use serde::{Deserialize, Serialize};

/// Create a new subject. This produces the geneisis event for a new subject.
/// The subject is created with the given schema and namespace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Create {
    request: StartRequest,
}
