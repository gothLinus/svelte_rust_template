use application::dto::UserDto;
use proto::v1;

use super::page_message;

page_message!(UserDto => v1::UserPage);
