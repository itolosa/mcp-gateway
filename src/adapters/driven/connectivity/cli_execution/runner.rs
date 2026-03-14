use std::collections::BTreeMap;

use tokio::io::AsyncWriteExt;

use crate::adapters::driven::configuration::model::CliOperationDef;
use crate::hexagon::ports::{
    CliOperationRunner, GatewayError, OperationCallRequest, OperationCallResult,
    OperationDescriptor,
};

pub struct ProcessCliRunner {
    tools: BTreeMap<String, CliOperationDef>,
}

impl ProcessCliRunner {
    pub fn new(tools: BTreeMap<String, CliOperationDef>) -> Self {
        Self { tools }
    }
}

impl CliOperationRunner for ProcessCliRunner {
    fn list_operations(&self) -> Vec<OperationDescriptor> {
        self.tools
            .iter()
            .map(|(name, def)| build_tool_descriptor(name, def))
            .collect()
    }

    fn has_operation(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    async fn call_operation(
        &self,
        request: &OperationCallRequest,
    ) -> Result<OperationCallResult, GatewayError> {
        let name = &request.name;
        let def = self
            .tools
            .get(name.as_str())
            .ok_or_else(|| GatewayError::CliOperation(format!("unknown CLI operation: {name}")))?;

        let input_json = request.arguments.as_deref().unwrap_or("{}");

        let output = run_command(&def.command, input_json).await.map_err(|e| {
            GatewayError::CliOperation(format!("failed to run '{}': {e}", def.command))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            Ok(OperationCallResult {
                content: vec![serde_json::json!({"type": "text", "text": stdout}).to_string()],
                is_error: false,
            })
        } else {
            Ok(OperationCallResult {
                content: vec![serde_json::json!({"type": "text", "text": stderr}).to_string()],
                is_error: true,
            })
        }
    }
}

async fn run_command(command: &str, input: &str) -> std::io::Result<std::process::Output> {
    let mut child = tokio::process::Command::new(command)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    #[allow(clippy::expect_used)]
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let _ = stdin.write_all(input.as_bytes()).await;
    drop(stdin);

    child.wait_with_output().await
}

fn build_tool_descriptor(name: &str, def: &CliOperationDef) -> OperationDescriptor {
    let description = def
        .description
        .clone()
        .unwrap_or_else(|| format!("Execute: {}", def.command));
    OperationDescriptor {
        name: name.to_string(),
        description: Some(description),
        schema: r#"{"type":"object"}"#.to_string(),
    }
}
