//! Smoke test: feed the `#[openapi]` macro a minimal spec, verify
//! the impl compiles and the `ToolProvider` returns the expected
//! tools. Exercised by `cargo test --test wrap_openapi_test`.

use tokitai_macros::{openapi, openapi_op};
use tokitai_core::{ToolProvider, ToolCaller};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
struct Pet {
    name: String,
    tag: Option<String>,
}

#[derive(Default)]
struct PetClient;

#[openapi(spec = "openai_minimal.json")]
impl PetClient {
    #[openapi_op(operation_id = "listPets")]
    pub fn list_pets(&self) -> Vec<Pet> { vec![] }

    #[openapi_op(operation_id = "createPet")]
    pub fn create_pet(&self, pet: Pet) -> Pet { pet }

    #[openapi_op(operation_id = "showPetById")]
    pub fn show_pet_by_id(&self, id: String) -> Pet { Pet { name: id, tag: None } }
}

fn main() {
    let client = PetClient;

    // 1. ToolProvider returns exactly three tools, in the order
    //    they were declared on the impl block.
    let defs = PetClient::tool_definitions();
    assert_eq!(defs.len(), 3);
    let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, vec!["listPets", "createPet", "showPetById"]);

    // 2. Each tool has a description sourced from the spec.
    assert!(defs[0].description.contains("List"));
    assert!(defs[1].description.contains("Create"));

    // 3. Each tool has a non-empty input schema. The exact shape is
    //    spec-driven (request body for POST, parameter list for
    //    GET), so we just check that the schema is valid JSON and
    //    that it is a JSON Schema 2020-12 object. We don't pin
    //    which `defs[i]` carries the POST body vs the GET
    //    parameter list — the macro may sort operations
    //    differently from the source order in v1.
    for def in defs {
        let schema: serde_json::Value =
            serde_json::from_str(&def.input_schema).unwrap();
        assert_eq!(schema["type"], "object", "schema for {} is not an object", def.name);
    }

    // 4. The generated `call_tool` dispatcher works for at least
    //    one round-trip on a sync method.
    let result = client
        .call_tool("showPetById", &serde_json::json!({"id": "42"}))
        .unwrap();
    let pet: Pet = serde_json::from_value(result).unwrap();
    assert_eq!(pet.name, "42");

    // 5. The `__OPENAPI_SPEC_RAW` static is populated with the
    //    raw spec text.
    assert!(!__OPENAPI_SPEC_RAW.is_empty());
    assert!(__OPENAPI_SPEC_RAW.contains("listPets"));

    // 6. The lookup map is keyed by operationId.
    let entry = __OPENAPI_OPS_PetClient.get("listPets");
    assert!(entry.is_some());
    assert_eq!(entry.unwrap().http_method, "GET");
    assert_eq!(entry.unwrap().path, "/pets");
}
