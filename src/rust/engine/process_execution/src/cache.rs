use crate::{ExecuteProcessRequest, FallibleExecuteProcessResult};
use boxfuture::{try_future, BoxFuture, Boxable};
use bytes::Bytes;
use futures::Future;
use hashing::{Digest, Fingerprint};
use protobuf::Message;
use sharded_lmdb::ShardedLmdb;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone)]
struct CommandRunner {
  underlying: Arc<dyn crate::CommandRunner>,
  store: ShardedLmdb,
}

impl crate::CommandRunner for CommandRunner {
  fn run(&self, req: ExecuteProcessRequest) -> BoxFuture<FallibleExecuteProcessResult, String> {
    let digest = try_future!(self.digest(&req));
    let key = digest.0;

    let command_runner = self.clone();
    self
      .lookup(key)
      .and_then(move |maybe_result| {
        if let Some(result) = maybe_result {
          futures::future::ok(result).to_boxed()
        } else {
          command_runner
            .underlying
            .run(req)
            .and_then(move |result| command_runner.store(key, &result).map(|()| result))
            .to_boxed()
        }
      })
      .to_boxed()
  }
}

impl CommandRunner {
  fn digest(&self, req: &ExecuteProcessRequest) -> Result<Digest, String> {
    // TODO: Parameterise options
    let (_action, _command, execute_request) =
      crate::remote::make_execute_request(req, &None, &None, BTreeMap::new())?;
    execute_request.get_action_digest().into()
  }

  fn lookup(
    &self,
    fingerprint: Fingerprint,
  ) -> impl Future<Item = Option<FallibleExecuteProcessResult>, Error = String> {
    self.store.load_bytes_with(fingerprint, |bytes| {
      let mut execute_response = bazel_protos::remote_execution::ExecuteResponse::new();
      execute_response
        .merge_from_bytes(&bytes)
        .map_err(|e| format!("Invalid ExecuteResponse: {:?}", e))?;
      unimplemented!();
      // TODO: Extract from remote::CommandRunner
      //            return self
      //                .extract_stdout(&execute_response)
      //                .join(self.extract_stderr(&execute_response))
      //                .join(self.extract_output_files(&execute_response))
      //                .and_then(move |((stdout, stderr), output_directory)| {
      //                    Ok(FallibleExecuteProcessResult {
      //                        stdout: stdout,
      //                        stderr: stderr,
      //                        exit_code: execute_response.get_result().get_exit_code(),
      //                        output_directory: output_directory,
      //                        execution_attempts: execution_attempts,
      //                    })
      //                })
    })
  }

  fn store(
    &self,
    fingerprint: Fingerprint,
    _result: &FallibleExecuteProcessResult,
  ) -> impl Future<Item = (), Error = String> {
    // TODO: Actually serialize the result
    self.store.store_bytes(fingerprint, Bytes::new(), false)
  }
}
