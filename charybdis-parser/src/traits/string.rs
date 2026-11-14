pub trait ToSnakeCase {
    fn to_snake_case(&self) -> String;
}

impl ToSnakeCase for str {
    fn to_snake_case(&self) -> String {
        let mut result = String::new();
        let mut prev_char_is_uppercase = false;

        for (i, c) in self.chars().enumerate() {
            if c.is_uppercase() {
                if i != 0 && !prev_char_is_uppercase {
                    result.push('_');
                }
                result.push(c.to_ascii_lowercase());
                prev_char_is_uppercase = true;
            } else {
                result.push(c);
                prev_char_is_uppercase = false;
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::ToSnakeCase;

    // the names of the generated macros (`find_<model>!`, `partial_<model>!`, ...) are built with this
    #[test]
    fn struct_names_become_snake_case() {
        assert_eq!("User".to_snake_case(), "user");
        assert_eq!("UpdateUsernameUser".to_snake_case(), "update_username_user");
        assert_eq!("PostsByCreator".to_snake_case(), "posts_by_creator");
    }

    #[test]
    fn consecutive_capitals_are_kept_together() {
        assert_eq!("UserID".to_snake_case(), "user_id");
        assert_eq!("HTTPRequest".to_snake_case(), "httprequest");
    }

    #[test]
    fn digits_and_existing_underscores_are_left_alone() {
        assert_eq!("Post2".to_snake_case(), "post2");
        assert_eq!("already_snake".to_snake_case(), "already_snake");
        assert_eq!("".to_snake_case(), "");
    }
}
