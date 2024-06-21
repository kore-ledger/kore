// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    db::Database,
    model::{request::EventRequest, signature::Signed},
    Error,
};

use actor::{Actor, ActorContext, Event, Handler, Message, Response};
use async_trait::async_trait;
use identity::identifier::DigestIdentifier;
use serde::{Deserialize, Serialize};
