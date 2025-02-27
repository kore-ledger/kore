// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Binary Codec
//! A "tell" codec using binary data for messasges.

use async_trait::async_trait;
use cbor4ii::core::error::DecodeError;
use futures::prelude::*;
use futures::{AsyncRead, AsyncWrite};
use libp2p::StreamProtocol;
use serde::{Serialize, de::DeserializeOwned};
use std::{
    collections::TryReserveError, convert::Infallible, io, marker::PhantomData,
};

/// Max request size in bytes
const REQUEST_SIZE_MAXIMUM: u64 = 1024 * 1024;

/// A codec for encoding and decoding messages using [`cbor`].
///
pub struct Codec<M> {
    pub max_message_size: u64,
    pub _phantom: PhantomData<M>,
}

impl<M> Default for Codec<M> {
    fn default() -> Self {
        Self {
            max_message_size: 1024 * 1024 * 8,
            _phantom: PhantomData,
        }
    }
}

impl<M> Clone for Codec<M> {
    fn clone(&self) -> Self {
        Self {
            max_message_size: self.max_message_size,
            _phantom: PhantomData,
        }
    }
}

#[async_trait]
impl<M> super::Codec for Codec<M>
where
    M: Serialize + DeserializeOwned + Send,
{
    type Protocol = StreamProtocol;
    type Message = M;

    async fn read_message<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Message>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut vec = Vec::new();
        io.take(REQUEST_SIZE_MAXIMUM).read_to_end(&mut vec).await?;

        cbor4ii::serde::from_slice(vec.as_slice()).map_err(decode_into_io_error)
    }

    async fn write_message<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        msg: Self::Message,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let data: Vec<u8> = cbor4ii::serde::to_vec(Vec::new(), &msg)
            .map_err(encode_into_io_error)?;

        io.write_all(data.as_ref()).await?;
        Ok(())
    }
}

fn decode_into_io_error(
    err: cbor4ii::serde::DecodeError<Infallible>,
) -> io::Error {
    match err {
        cbor4ii::serde::DecodeError::Core(DecodeError::Read(e)) => {
            io::Error::new(io::ErrorKind::Other, e)
        }
        cbor4ii::serde::DecodeError::Core(
            e @ DecodeError::Unsupported { .. },
        ) => io::Error::new(io::ErrorKind::Unsupported, e),
        cbor4ii::serde::DecodeError::Core(e @ DecodeError::Eof { .. }) => {
            io::Error::new(io::ErrorKind::UnexpectedEof, e)
        }
        cbor4ii::serde::DecodeError::Core(e) => {
            io::Error::new(io::ErrorKind::InvalidData, e)
        }
        cbor4ii::serde::DecodeError::Custom(e) => {
            io::Error::new(io::ErrorKind::Other, e.to_string())
        }
    }
}

fn encode_into_io_error(
    err: cbor4ii::serde::EncodeError<TryReserveError>,
) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::Codec;
    use async_std::io::Cursor;
    use futures::executor::block_on;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
    struct TestMessage {
        a: u32,
        b: String,
    }

    #[test]
    fn test_codec() {
        let mut codec = super::Codec::default();
        let msg = TestMessage {
            a: 42,
            b: "hello".to_string(),
        };

        let mut buf = Vec::new();
        block_on(codec.write_message(
            &StreamProtocol::new("/test"),
            &mut buf,
            msg.clone(),
        ))
        .unwrap();

        let mut buf = Cursor::new(buf);
        let msg2 = block_on(
            codec.read_message(&StreamProtocol::new("/test"), &mut buf),
        )
        .unwrap();

        assert_eq!(msg, msg2);
    }
}
