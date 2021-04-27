// Copyright 2021 Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.
//

//! Error handling related code and Error/Result definitions.

use polkadot_node_network_protocol::request_response::request::RequestError;
use polkadot_primitives::v1::SessionIndex;
use thiserror::Error;

use futures::channel::oneshot;

use polkadot_node_subsystem_util::{Fault, Error as UtilError, runtime, unwrap_non_fatal};
use polkadot_subsystem::{errors::RuntimeApiError, SubsystemError};

use crate::LOG_TARGET;

#[derive(Debug, Error)]
#[error(transparent)]
pub struct Error(pub Fault<NonFatal, Fatal>);

impl From<NonFatal> for Error {
	fn from(e: NonFatal) -> Self {
		Self(Fault::from_non_fatal(e))
	}
}

impl From<Fatal> for Error {
	fn from(f: Fatal) -> Self {
		Self(Fault::from_fatal(f))
	}
}

impl From<runtime::Error> for Error {
	fn from(o: runtime::Error) -> Self {
		Self(Fault::from_other(o))
	}
}

/// Fatal errors of this subsystem.
#[derive(Debug, Error)]
pub enum Fatal {
	/// Spawning a running task failed.
	#[error("Spawning subsystem task failed")]
	SpawnTask(#[source] SubsystemError),

	/// Runtime API subsystem is down, which means we're shutting down.
	#[error("Runtime request canceled")]
	RuntimeRequestCanceled(oneshot::Canceled),

	/// Requester stream exhausted.
	#[error("Erasure chunk requester stream exhausted")]
	RequesterExhausted,

	#[error("Receive channel closed")]
	IncomingMessageChannel(#[source] SubsystemError),

	/// Errors coming from runtime::Runtime.
	#[error("Error while accessing runtime information")]
	Runtime(#[from] #[source] runtime::Fatal),
}

/// Non fatal errors of this subsystem.
#[derive(Debug, Error)]
pub enum NonFatal {
	/// av-store will drop the sender on any error that happens.
	#[error("Response channel to obtain chunk failed")]
	QueryChunkResponseChannel(#[source] oneshot::Canceled),

	/// av-store will drop the sender on any error that happens.
	#[error("Response channel to obtain available data failed")]
	QueryAvailableDataResponseChannel(#[source] oneshot::Canceled),

	/// We tried accessing a session that was not cached.
	#[error("Session is not cached.")]
	NoSuchCachedSession,

	/// We tried reporting bad validators, although we are not a validator ourselves.
	#[error("Not a validator.")]
	NotAValidator,

	/// Sending request response failed (Can happen on timeouts for example).
	#[error("Sending a request's response failed.")]
	SendResponse,

	/// Some request to utility functions failed.
	/// This can be either `RuntimeRequestCanceled` or `RuntimeApiError`.
	#[error("Utility request failed")]
	UtilRequest(UtilError),

	/// Some request to the runtime failed.
	/// For example if we prune a block we're requesting info about.
	#[error("Runtime API error")]
	RuntimeRequest(RuntimeApiError),

	/// Fetching PoV failed with `RequestError`.
	#[error("FetchPoV request error")]
	FetchPoV(#[source] RequestError),

	/// Fetching PoV failed as the received PoV did not match the expected hash.
	#[error("Fetched PoV does not match expected hash")]
	UnexpectedPoV,

	#[error("Remote responded with `NoSuchPoV`")]
	NoSuchPoV,

	/// No validator with the index could be found in current session.
	#[error("Given validator index could not be found")]
	InvalidValidatorIndex,

	/// We tried fetching a session info which was not available.
	#[error("There was no session with the given index")]
	NoSuchSession(SessionIndex),

	/// Errors coming from runtime::Runtime.
	#[error("Error while accessing runtime information")]
	Runtime(#[from] #[source] runtime::NonFatal),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Utility for eating top level errors and log them.
///
/// We basically always want to try and continue on error. This utility function is meant to
/// consume top-level errors by simply logging them
pub fn log_error(result: Result<()>, ctx: &'static str)
	-> std::result::Result<(), Fatal>
{
	if let Some(error) = unwrap_non_fatal(result.map_err(|e| e.0))? {
		tracing::warn!(target: LOG_TARGET, error = ?error, ctx);
	}
	Ok(())
}

/// Receive a response from a runtime request and convert errors.
pub(crate) async fn recv_runtime<V>(
	r: oneshot::Receiver<std::result::Result<V, RuntimeApiError>>,
) -> Result<V> {
	let result = r.await
		.map_err(Fatal::RuntimeRequestCanceled)?
		.map_err(NonFatal::RuntimeRequest)?;
	Ok(result)
}
