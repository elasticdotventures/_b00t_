from .helpers import AwsHelper

# This is a placeholder for the actual AbstractToolGenerator.
# In a real implementation, this would be imported from the b00t MCP framework.
class AbstractToolGenerator:
    def __init__(self, tool_name: str, pretty_name: str, description: str):
        self.tool_name = tool_name
        self.pretty_name = pretty_name
        self.description = description
    def generate_tools(self) -> list:
        raise NotImplementedError

class AWSToolGenerator(AbstractToolGenerator):
    """
    Generates MCP tools for AWS services using a shared, unified AwsHelper.
    """
    # Use a single, shared helper instance for all tool generators.
    # This ensures the client cache is shared across all instances.
    aws_helper = AwsHelper()

    def __init__(self, service_name: str):
        self.service_name = service_name
        # get_client will return a cached client if available
        self.client = self.aws_helper.get_client(service_name)
        self.service_model = self.client.meta.service_model

        super().__init__(
            tool_name=f"aws-{service_name}",
            pretty_name=f"AWS {self.service_model.service_id}",
            description=self.service_model.documentation,
        )

    def generate_tools(self) -> list:
        """
        [Placeholder] In a real implementation, this method would inspect
        the boto3 client's service model and generate a list of
        MCP-compatible tools for each available action.
        """
        print(f"Generating tools for {self.pretty_name}...")
        # Example: iterate over client.meta.service_model.operation_names
        return []
