pub mod dto;
pub mod error;
pub mod schema;
pub mod schema_gen;

pub use dto::*;
pub use error::{VacuaErrorCode, VacuaErrorResponse};
pub use schema::*;
pub use schema_gen::generate_all_schemas;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_generation_is_valid() {
        let schemas = generate_all_schemas();
        assert_eq!(schemas.len(), 18);
        for (name, schema) in schemas {
            assert!(schema.is_object(), "schema {} must be a JSON object", name);
            assert!(
                schema.get("$schema").is_some() || schema.get("title").is_some(),
                "schema {} should have schema metadata",
                name
            );
        }
    }
}
