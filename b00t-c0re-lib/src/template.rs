//! Template rendering functionality for b00t ecosystem
//!
//! Provides simple string-based template rendering with b00t-specific variables
//! including PID, timestamps, git info, and agent context.

use crate::B00tResult;
use crate::context::B00tContext;
use anyhow::Context;

/// Template renderer that handles b00t-specific variable substitution
#[derive(Debug, Clone)]
pub struct TemplateRenderer {
    context: B00tContext,
}

impl TemplateRenderer {
    /// Create a new template renderer with the given context
    pub fn new(context: B00tContext) -> Self {
        Self { context }
    }

    /// Create a template renderer with default context
    pub fn with_defaults() -> B00tResult<Self> {
        let context = B00tContext::current()?;
        Ok(Self::new(context))
    }

    /// Render a template string by replacing b00t variables
    ///
    /// Supports these template variables:
    /// - {{PID}} - Current process ID
    /// - {{TIMESTAMP}} - Current UTC timestamp
    /// - {{USER}} - Current username
    /// - {{BRANCH}} - Current git branch
    /// - {{_B00T_Agent}} / {{_B00T_AGENT}} - Agent identifier
    /// - {{MODEL_SIZE}} - Model size identifier
    /// - {{PRIVACY}} - Privacy setting
    /// - {{WORKSPACE_ROOT}} - Workspace root directory
    /// - {{IS_GIT_REPO}} / {{GIT_REPO}} - Is git repository (true/false)
    /// - {{HOSTNAME}} - System hostname
    pub fn render(&self, template: &str) -> B00tResult<String> {
        let mut rendered = template.to_string();

        // Replace all b00t template variables
        rendered = rendered.replace("{{PID}}", &self.context.pid.to_string());
        rendered = rendered.replace("{{TIMESTAMP}}", &self.context.timestamp);
        rendered = rendered.replace("{{USER}}", &self.context.user);
        rendered = rendered.replace("{{BRANCH}}", &self.context.branch);
        rendered = rendered.replace("{{_B00T_Agent}}", &self.context.agent);
        rendered = rendered.replace("{{_B00T_AGENT}}", &self.context.agent);
        rendered = rendered.replace("{{MODEL_SIZE}}", &self.context.model_size);
        rendered = rendered.replace("{{PRIVACY}}", &self.context.privacy);
        rendered = rendered.replace("{{WORKSPACE_ROOT}}", &self.context.workspace_root);
        rendered = rendered.replace("{{IS_GIT_REPO}}", &self.context.is_git_repo.to_string());
        rendered = rendered.replace("{{GIT_REPO}}", &self.context.is_git_repo.to_string());
        rendered = rendered.replace("{{HOSTNAME}}", &self.context.hostname);

        Ok(rendered)
    }

    /// Render a template from a file path
    pub fn render_file<P: AsRef<std::path::Path>>(&self, path: P) -> B00tResult<String> {
        let template_content = std::fs::read_to_string(&path).with_context(|| {
            format!("Failed to read template file: {}", path.as_ref().display())
        })?;

        self.render(&template_content)
    }

    /// Update the context used for rendering
    pub fn set_context(&mut self, context: B00tContext) {
        self.context = context;
    }

    /// Get a reference to the current context
    pub fn context(&self) -> &B00tContext {
        &self.context
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_rendering() {
        let context = B00tContext {
            pid: 12345,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            user: "testuser".to_string(),
            branch: "main".to_string(),
            agent: "TestAgent".to_string(),
            model_size: "large".to_string(),
            privacy: "standard".to_string(),
            workspace_root: "/tmp/test".to_string(),
            is_git_repo: true,
            hostname: "testhost".to_string(),
        };

        let renderer = TemplateRenderer::new(context);

        let template = "PID: {{PID}}, User: {{USER}}, Branch: {{BRANCH}}, Agent: {{_B00T_Agent}}";
        let result = renderer.render(template).unwrap();

        assert_eq!(
            result,
            "PID: 12345, User: testuser, Branch: main, Agent: TestAgent"
        );
    }

    #[test]
    fn test_git_repo_rendering() {
        let context = B00tContext {
            pid: 123,
            timestamp: "now".to_string(),
            user: "user".to_string(),
            branch: "main".to_string(),
            agent: "Agent".to_string(),
            model_size: "small".to_string(),
            privacy: "private".to_string(),
            workspace_root: "/workspace".to_string(),
            is_git_repo: false,
            hostname: "host".to_string(),
        };

        let renderer = TemplateRenderer::new(context);
        let template = "Git repo: {{IS_GIT_REPO}} / {{GIT_REPO}}";
        let result = renderer.render(template).unwrap();

        assert_eq!(result, "Git repo: false / false");
    }
}
