# terraform.🧊/

b00t uses `tofu` and a series of standards conventions;

there is a bash alias `tf` or just `tofu'`

# best practices:

MUST ALWAYS:
* use tiered modularized composition approach, there is a global 'remote state' which holds credentials, common modules, use that (don't duplicate)
* use context7 to view terraform docs
* each discrete application, infrastructure or micro-service SHOULD have it's own state file
* Data source performs a read-only operation and is dependant on provider configuration, it is used in a resource module and an infrastructure module.
* Data source terraform_remote_state acts as a glue for higher-level modules and compositions.
* resource names use singular nouns, 
* Include argument tags, if supported by resource, as the last real argument, 
* following by depends_on and lifecycle, if necessary. All of these should be separated by a single empty line.


MUST NEVER:
* downgrade modules unless explicitly instructed
* repeat resource type in resource name (not partially, nor completely),


RECOMMEND:
* run `tf fmt` 