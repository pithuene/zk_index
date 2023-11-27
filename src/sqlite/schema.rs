// @generated automatically by Diesel CLI.

diesel::table! {
    file (path) {
        path -> Text,
        last_indexed -> Integer,
    }
}

diesel::table! {
    note (file) {
        vault_path -> Text,
        file -> Text,
    }
}

diesel::joinable!(note -> file (file));

diesel::allow_tables_to_appear_in_same_query!(file, note,);
