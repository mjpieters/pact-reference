use std::{
  env,
  fs,
  path::Path
};
use std::path::PathBuf;

use expectest::prelude::*;
use pact_models::pact::ReadWritePact;
use pact_models::sync_pact::RequestResponsePact;
use rand::prelude::*;
use reqwest::Client;
use serde_json::json;

use pact_consumer::{json_pattern, json_pattern_internal, like};
use pact_consumer::prelude::*;

/// This is supposed to be a doctest in mod, but it's breaking there, so
/// we have an executable copy here.
///
/// This test is currently ignored because it has a race condition when running in CI. Probably
/// because it is mutating environment variables that point to directories on disk
#[test_log::test(tokio::test)]
async fn mock_server_passing_validation() {
    let output_dir = output_dir("target/pact_dir");

    env::set_var("PACT_OUTPUT_DIR", &output_dir);

    {
      // Define the Pact for the test, specify the names of the consuming
      // application and the provider application.
      let alice_service = PactBuilder::new_v4("Consumer", "Alice Service")
        // Start a new interaction. We can add as many interactions as we want.
        .interaction("a retrieve Mallory request", "", |mut i| {
          // Defines a provider state. It is optional.
          i.given("there is some good mallory");
          // Define the request, a GET (default) request to '/mallory'.
          i.request.path("/mallory");
          i.request.header("content-type", "application/json");
          // Define the response we want returned.
          i.response
            .ok()
            .content_type("text/plain")
            .body("That is some good Mallory.");

          // Return the interaction back to the pact framework
          i.clone()
        })
        .start_mock_server(None);

      // You would use your actual client code here.
      let mallory_url = alice_service.path("/mallory");
      let client = reqwest::Client::new();
      let response = client.get(mallory_url).header("content-type", "application/json").send().await.expect("could not fetch URL");
      let body = response.text().await.expect("could not read response body");
      assert_eq!(body, "That is some good Mallory.");
    }

    // When your test has finished running, all verifications will be performed
    // automatically, and an error will be thrown if any have failed.

    env::remove_var("PACT_OUTPUT_DIR");

    let path_file = Path::new("target/pact_dir/Consumer-Alice Service.json");
    expect!(path_file.exists()).to(be_true());
}

fn output_dir(path: &str) -> PathBuf {
  match Path::new(path).canonicalize() {
    Ok(path) => {
      fs::remove_dir_all(path.clone()).unwrap_or(());
      path
    }
    Err(_) => {
      let path = Path::new(path);
      fs::create_dir_all(path.clone()).unwrap_or(());
      path.canonicalize().unwrap()
    }
  }
}

#[test_log::test(tokio::test)]
async fn mock_server_passing_validation_async_version() {
  // Define the Pact for the test, specify the names of the consuming
  // application and the provider application.
  let alice_service = PactBuilderAsync::new("Consumer", "Alice Service")
    // Start a new interaction. We can add as many interactions as we want.
    .interaction("a retrieve Mallory request", "", |mut i| async move {
      // Defines a provider state. It is optional.
      i.given("there is some good mallory");
      // Define the request, a GET (default) request to '/mallory'.
      i.request.path("/mallory");
      // Define the response we want returned.
      i.response
        .ok()
        .content_type("text/plain")
        .body("That is some good Mallory.");

      // Return the interaction back to the pact framework
      i.clone()
    })
    .await
    .start_mock_server(None);

  // You would use your actual client code here.
  let mallory_url = alice_service.path("/mallory");
  let response = reqwest::get(mallory_url).await.expect("could not fetch URL");
  let body = response.text().await.expect("could not read response body");
  assert_eq!(body, "That is some good Mallory.");
}

#[test_log::test]
#[should_panic]
fn mock_server_failing_validation() {
    let hello_service = PactBuilder::new("Hello CLI", "Hello Server")
        .interaction("request a greeting", "", |mut i| {
          i.request.path("/hello");
          i.response.body("Hello!");
          i.clone()
        })
      .start_mock_server(None);
    // Call with the wrong URL, which should lead to a panic at the end of
    // the function.
    let url = hello_service.path("/goodbye");
    let _ = reqwest::blocking::get(url);
}

#[test_log::test(tokio::test)]
async fn duplicate_interactions() {
  let u8 = random::<u8>();
  let output_dir = output_dir(&*format!("target/pact_dir_{:03}", u8));

  for _ in 1..=3 {
    let mock_service = PactBuilder::new("consumer 1", "provider 1")
      .interaction("tricky test", "", |mut interaction| {
        interaction
          .request
          .put()
          .json_body(pact_consumer::json_pattern!({
                          "name": pact_consumer::like!("mai"),
                          "street": pact_consumer::like!("5th"),
                          "state": pact_consumer::like!("VA"),
                      }))
          .path("/rolex.html");
        interaction.response.body("TrixR4Kidz");
        interaction
      })
      .output_dir(&output_dir)
      .start_mock_server(None);

    let mock_url = mock_service.url();

    assert_eq!(
      Client::new()
        .put(&format!("{}rolex.html", mock_url))
        .json(&serde_json::json!({
                      "name": "mai",
                      "street": "5th",
                      "state": "VA",
                  }))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap(),
      "TrixR4Kidz",
    );
  }

  let path = output_dir.join("consumer 1-provider 1.json");
  let written_pact = RequestResponsePact::read_pact(path.as_path()).unwrap();
  let _ = fs::remove_dir_all(output_dir);

  expect!(written_pact.interactions.len()).to(be_equal_to(1));
}

#[test_log::test(tokio::test)]
async fn test_two_interactions() {
  {
    let mock_service = PactBuilder::new("test_two_interactions_consumer", "test_two_interactions_provider")
      .interaction("looks for something that doesn't exist", "", |mut i| {
        i.request
          .post()
          .path("/")
          .content_type("application/json")
          .json_body(like!(json!({"key": "i_dont_exist"})));
        i.response
          .content_type("application/json")
          .json_body(json!({"count": 0, "results": []}));
        i
      })
      .start_mock_server(None);

    let mock_url = mock_service.url();
    Client::new().post(mock_url).json(&json!({"key": "i_dont_exist"})).send().await.unwrap();
  }

  {
    let mock_service = PactBuilder::new("test_two_interactions_consumer", "test_two_interactions_provider")
      .interaction("looks for something that exists", "", |mut i| {
        i.request
          .post()
          .path("/")
          .content_type("application/json")
          .json_body(like!(json!({"key": "i_exist"})));
        i.response
          .content_type("application/json")
          .json_body(json!({"count": 1, "results": ["i_exist"]}));
        i
      })
      .start_mock_server(None);

    let mock_url = mock_service.url();
    Client::new().post(mock_url).json(&json!({"key": "i_exist"})).send().await.unwrap();
  }

  let path_file = Path::new("target/pacts/test_two_interactions_consumer-test_two_interactions_provider.json");
  expect!(path_file.exists()).to(be_true());

  let pact = RequestResponsePact::read_pact(&path_file).unwrap();
  expect!(pact.interactions.len()).to(be_equal_to(2));
}
