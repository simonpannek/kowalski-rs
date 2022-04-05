// Info messages
pub const INFO_CMD_GLOBAL: &str = "Global commands registered.";
pub const INFO_CMD_MODULE: &str = "Module commands registered.";
pub const INFO_CONNECTED: &str = "Connection to Discord API established!";
pub const INFO_DB_CONNECTED: &str = "Database connection established.";
pub const INFO_DB_SETUP: &str = "Database setup complete.";
// Error messages
pub const ERR_API_LOAD: &str = "Failed to request information from the REST API";
pub const ERR_AUTOCOMPLETE: &str = "Failed to answer the autocomplete request";
pub const ERR_CLIENT: &str = "Client error";
pub const ERR_CMD_ARGS_INVALID: &str = "The argument provided is invalid";
pub const ERR_CMD_ARGS_LENGTH: &str = "Could not find required argument";
pub const ERR_CMD_ARGS_TYPE: &str = "The argument provided has an unexpected type";
pub const ERR_CMD_CREATION: &str = "Failed to create bot commands";
pub const ERR_CMD_EXECUTION: &str = "Failed to execute the command";
pub const ERR_CMD_RESPONSE_INVALID: &str = "The response provided is invalid";
pub const ERR_CMD_SEND_FAILURE: &str = "Failed to send the failure notification";
pub const ERR_CMD_NOT_FOUND: &str = "Failed to find the command in the config";
pub const ERR_CMD_SET_PERMISSION: &str = "Failed to set command permissions";
pub const ERR_CMD_PERMISSION: &str =
    "A user with insufficient permissions tried to execute the command";
pub const ERR_CONFIG_PARSE: &str = "Failed to parse config file";
pub const ERR_CONFIG_READ: &str = "Failed to read config file";
pub const ERR_DATA_ACCESS: &str = "Failed to access the global data";
pub const ERR_DB_CONNECTION: &str = "Database connection error";
pub const ERR_DB_QUERY: &str = "Failed to execute the database query";
pub const ERR_ENV_NOT_SET: &str = "Environment variable not set";
pub const ERR_MEMBER_REMOVAL: &str = "Failed to handle the member removal event";
pub const ERR_MESSAGE_COMPONENT: &str = "Failed to answer the message component request";
pub const ERR_REACTION: &str = "Failed to handle the reaction event";
// User error messages
pub const ERR_USER_TITLE: &str = "Looks like something really went wrong here :/";
pub const ERR_USER_EXECUTION_FAILED: &str =
    "You may want to reach out to the owner of this bot to check what went wrong.";
