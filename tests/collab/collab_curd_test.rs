use crate::util::test_client::TestClient;
use app_error::ErrorCode;
use assert_json_diff::assert_json_include;
use collab::core::collab_plugin::EncodedCollab;
use collab_entity::CollabType;
use database_entity::dto::{
  BatchCreateCollabParams, CollabParams, CreateCollabParams, QueryCollab, QueryCollabParams,
  QueryCollabResult,
};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

use reqwest::Method;
use serde::Serialize;
use serde_json::json;

use shared_entity::response::AppResponse;
use uuid::Uuid;

#[tokio::test]
async fn batch_insert_collab_with_empty_payload_test() {
  let mut test_client = TestClient::new_user().await;
  let workspace_id = test_client.workspace_id().await;

  let error = test_client
    .create_collab_list(&workspace_id, vec![])
    .await
    .unwrap_err();

  assert_eq!(error.code, ErrorCode::InvalidRequest);
}

#[tokio::test]
async fn batch_insert_collab_success_test() {
  let mut test_client = TestClient::new_user().await;
  let workspace_id = test_client.workspace_id().await;

  let mock_encoded_collab_v1 = vec![
    create_random_bytes(100 * 1024),
    create_random_bytes(300 * 1024),
    create_random_bytes(600 * 1024),
    create_random_bytes(800 * 1024),
    create_random_bytes(1024 * 1024),
  ];

  let params_list = (0..5)
    .map(|i| CollabParams {
      object_id: Uuid::new_v4().to_string(),
      encoded_collab_v1: mock_encoded_collab_v1[i].clone(),
      collab_type: CollabType::Document,
      override_if_exist: false,
    })
    .collect::<Vec<_>>();

  test_client
    .create_collab_list(&workspace_id, params_list.clone())
    .await
    .unwrap();

  let params = params_list
    .iter()
    .map(|params| QueryCollab {
      object_id: params.object_id.clone(),
      collab_type: params.collab_type.clone(),
    })
    .collect::<Vec<_>>();

  let result = test_client
    .batch_get_collab(&workspace_id, params)
    .await
    .unwrap();

  for params in params_list {
    let encoded_collab = result.0.get(&params.object_id).unwrap();
    match encoded_collab {
      QueryCollabResult::Success { encode_collab_v1 } => {
        assert_eq!(encode_collab_v1, &params.encoded_collab_v1)
      },
      QueryCollabResult::Failed { .. } => {
        panic!("Failed to get collab");
      },
    }
  }

  assert_eq!(result.0.values().len(), 5);
}

#[tokio::test]
async fn create_collab_params_compatibility_serde_test() {
  // This test is to make sure that the CreateCollabParams is compatible with the old InsertCollabParams
  let old_version_value = json!(InsertCollabParams {
    object_id: "object_id".to_string(),
    encoded_collab_v1: vec![0, 200],
    workspace_id: "workspace_id".to_string(),
    collab_type: CollabType::Document,
  });

  let new_version_create_params =
    serde_json::from_value::<CreateCollabParams>(old_version_value.clone()).unwrap();

  let new_version_value = serde_json::to_value(new_version_create_params.clone()).unwrap();
  assert_json_include!(actual: new_version_value.clone(), expected: old_version_value.clone());

  assert_eq!(new_version_create_params.object_id, "object_id".to_string());
  assert_eq!(new_version_create_params.encoded_collab_v1, vec![0, 200]);
  assert_eq!(
    new_version_create_params.workspace_id,
    "workspace_id".to_string()
  );
  assert_eq!(new_version_create_params.collab_type, CollabType::Document);
}

#[derive(Serialize)]
struct InsertCollabParams {
  pub object_id: String,
  pub encoded_collab_v1: Vec<u8>,
  pub workspace_id: String,
  pub collab_type: CollabType,
}

#[tokio::test]
async fn create_collab_compatibility_with_json_params_test() {
  let test_client = TestClient::new_user().await;
  let workspace_id = test_client.workspace_id().await;
  let object_id = Uuid::new_v4().to_string();
  let api_client = &test_client.api_client;
  let url = format!(
    "{}/api/workspace/{}/collab/{}",
    api_client.base_url, workspace_id, &object_id
  );

  let encoded_collab = EncodedCollab::new_v1(vec![0, 1, 2, 3, 4, 5, 6], vec![7, 8, 9, 10]);
  let params = OldCreateCollabParams {
    inner: CollabParams {
      object_id: object_id.clone(),
      encoded_collab_v1: encoded_collab.encode_to_bytes().unwrap(),
      collab_type: CollabType::Document,
      override_if_exist: false,
    },
    workspace_id: workspace_id.clone(),
  };

  test_client
    .api_client
    .http_client_with_auth(Method::POST, &url)
    .await
    .unwrap()
    .json(&params)
    .send()
    .await
    .unwrap();

  let resp = test_client
    .api_client
    .http_client_with_auth(Method::GET, &url)
    .await
    .unwrap()
    .json(&QueryCollabParams {
      workspace_id,
      inner: QueryCollab {
        object_id: object_id.clone(),
        collab_type: CollabType::Document,
      },
    })
    .send()
    .await
    .unwrap();

  let encoded_collab_from_server = AppResponse::<EncodedCollab>::from_response(resp)
    .await
    .unwrap()
    .into_data()
    .unwrap();
  assert_eq!(encoded_collab, encoded_collab_from_server);
}

#[tokio::test]
async fn batch_create_collab_compatibility_with_uncompress_params_test() {
  let test_client = TestClient::new_user().await;
  let workspace_id = test_client.workspace_id().await;
  let object_id = Uuid::new_v4().to_string();
  let api_client = &test_client.api_client;
  let url = format!(
    "{}/api/workspace/{}/collabs",
    api_client.base_url, workspace_id,
  );

  let encoded_collab = EncodedCollab::new_v1(vec![0, 1, 2, 3, 4, 5, 6], vec![7, 8, 9, 10]);
  let params = BatchCreateCollabParams {
    workspace_id: workspace_id.to_string(),
    params_list: vec![CollabParams {
      object_id: object_id.clone(),
      encoded_collab_v1: encoded_collab.encode_to_bytes().unwrap(),
      collab_type: CollabType::Document,
      override_if_exist: false,
    }],
  }
  .to_bytes()
  .unwrap();

  test_client
    .api_client
    .http_client_with_auth(Method::POST, &url)
    .await
    .unwrap()
    .body(params)
    .send()
    .await
    .unwrap();

  let url = format!(
    "{}/api/workspace/{}/collab/{}",
    api_client.base_url, workspace_id, &object_id
  );
  let resp = test_client
    .api_client
    .http_client_with_auth(Method::GET, &url)
    .await
    .unwrap()
    .json(&QueryCollabParams {
      workspace_id,
      inner: QueryCollab {
        object_id: object_id.clone(),
        collab_type: CollabType::Document,
      },
    })
    .send()
    .await
    .unwrap();

  let encoded_collab_from_server = AppResponse::<EncodedCollab>::from_response(resp)
    .await
    .unwrap()
    .into_data()
    .unwrap();
  assert_eq!(encoded_collab, encoded_collab_from_server);
}

#[derive(Debug, Clone, Serialize)]
pub struct OldCreateCollabParams {
  #[serde(flatten)]
  inner: CollabParams,
  pub workspace_id: String,
}

fn create_random_bytes(size: usize) -> Vec<u8> {
  let s: String = thread_rng()
    .sample_iter(&Alphanumeric)
    .take(size)
    .map(char::from)
    .collect();
  s.into_bytes()
}
