//! The `plugins` module provides exported functions using C bindings for using plugins with
//! Pact tests.

use std::os::raw::{c_char, c_uint};

use anyhow::{anyhow, Context};
use bytes::Bytes;
use log::{debug, error};
use pact_plugin_driver::catalogue_manager::find_content_matcher;
use pact_plugin_driver::content::PluginConfiguration;
use pact_plugin_driver::plugin_manager::{drop_plugin_access, load_plugin};
use pact_plugin_driver::plugin_models::{PluginDependency, PluginDependencyType};
use serde_json::Value;

use pact_models::bodies::OptionalBody;
use pact_models::content_types::ContentType;
use pact_models::http_parts::HttpPart;
use pact_models::json_utils::body_from_json;
use pact_models::pact::Pact;
use pact_models::plugins::PluginData;
use pact_models::v4::interaction::{InteractionMarkup, V4Interaction};
use pact_models::v4::synch_http::SynchronousHttp;
use pact_models::v4::V4InteractionType;

use crate::{ffi_fn, safe_str};
use crate::error::{catch_panic, set_error_msg};
use crate::mock_server::handles::{InteractionHandle, InteractionPart, PactHandle};
use crate::string::if_null;

ffi_fn! {
  /// Add a plugin to be used by the test. The plugin needs to be installed correctly for this
  /// function to work.
  ///
  /// * `plugin_name` is the name of the plugin to load.
  /// * `plugin_version` is the version of the plugin to load. It is optional, and can be NULL.
  ///
  /// Returns zero on success, and a positive integer value on failure.
  ///
  /// Note that plugins run as seperate processes, so will need to be cleaned up afterwards by
  /// calling `pactffi_cleanup_plugins` otherwise you have plugin processes left running.
  ///
  /// # Safety
  ///
  /// `plugin_name` must be a valid pointer to a NULL terminated string. `plugin_version` may be null,
  /// and if not NULL must also be a valid pointer to a NULL terminated string.
  ///
  /// # Errors
  ///
  /// * `1` - A general panic was caught.
  /// * `2` - Failed to load the plugin.
  /// * `3` - Pact Handle is not valid.
  ///
  /// When an error errors, LAST_ERROR will contain the error message.
  fn pactffi_using_plugin(pact: PactHandle, plugin_name: *const c_char, plugin_version: *const c_char) -> c_uint {
    let plugin_name = safe_str!(plugin_name);
    let plugin_version = if_null(plugin_version, "");

     let runtime = tokio::runtime::Runtime::new().unwrap();
     let result = runtime.block_on(load_plugin(&PluginDependency {
        name: plugin_name.to_string(),
        version: if plugin_version.is_empty() { None } else { Some(plugin_version) },
        dependency_type: Default::default()
      }));
    match result {
      Ok(plugin) => pact.with_pact(&|_, inner| {
        inner.pact.add_plugin(plugin.manifest.name.as_str(), plugin.manifest.version.as_str(), None)
          .expect("Could not add plugin to pact");
        0
      }).unwrap_or(3),
      Err(err) => {
        error!("Could not load plugin - {}", err);
        set_error_msg(format!("Could not load plugin - {}", err));
        2
      }
    }
  } {
    1
  }
}

ffi_fn! {
  /// Decrement the access count on any plugins that are loaded for the Pact. This will shutdown
  /// any plugins that are no longer required (access count is zero).
  fn pactffi_cleanup_plugins(pact: PactHandle) {
    pact.with_pact(&|_, inner| {
      // decrement access to any plugin loaded for the Pact
      for plugin in inner.pact.plugin_data() {
        let dependency = PluginDependency {
          name: plugin.name,
          version: Some(plugin.version),
          dependency_type: PluginDependencyType::Plugin
        };
        drop_plugin_access(&dependency);
      }
    });
  }
}

/// Setup the interaction part using a plugin. The contents is a JSON string that will be passed on to
/// the plugin to configure the interaction part. Refer to the plugin documentation on the format
/// of the JSON contents.
///
/// Returns zero on success, and a positive integer value on failure.
///
/// * `interaction` - Handle to the interaction to configure.
/// * `part` - The part of the interaction to configure (request or response).
/// * `content_type` - NULL terminated C string of the content type of the part.
/// * `contents` - NULL terminated C string of the JSON contents that gets passed to the plugin.
///
/// # Safety
///
/// `content_type` and `contents` must be a valid pointers to NULL terminated strings.
///
/// # Errors
///
/// * `1` - A general panic was caught.
/// * `2` - The mock server has already been started.
/// * `3` - The interaction handle is invalid.
/// * `4` - The content type is not valid.
/// * `5` - The contents JSON is not valid JSON.
/// * `6` - The plugin returned an error.
///
/// When an error errors, LAST_ERROR will contain the error message.
#[no_mangle]
pub extern fn pactffi_interaction_contents(interaction: InteractionHandle, part: InteractionPart, content_type: *const c_char, contents: *const c_char) -> c_uint {
  catch_panic(|| {
    let content_type_str = safe_str!(content_type);
    let content_type = match ContentType::parse(content_type_str) {
      Ok(ct) => ct,
      Err(err) => {
        error!("'{}' is not a valid content type - {}", content_type_str, err);
        set_error_msg(format!("'{}' is not a valid content type - {}", content_type_str, err));
        return Ok(4);
      }
    };

    let contents_str = safe_str!(contents);
    let contents = match serde_json::from_str(contents_str) {
      Ok(value) => value,
      Err(err) => {
        error!("Contents is not a valid JSON - {}", err);
        set_error_msg(format!("Contents is not a valid JSON - {}", err));
        return Ok(5);
      }
    };

    let result = interaction.with_interaction(&|_, started, inner| {
      if !started {
        match inner.v4_type() {
          V4InteractionType::Synchronous_HTTP => {
            setup_contents(inner.as_v4_http_mut().unwrap(), part, &content_type, &contents)
          }
          _ => todo!("{} type of interaction is not supported yet", inner.v4_type())
        }
      } else {
        Err(anyhow!("Mock server is already started"))
      }
    });

    match result {
      Some(value) => match value {
        Ok(plugin_config) => {
          if let Some((plugin, version, config)) = plugin_config {
            interaction.with_pact(&|_, pact| {
              pact.pact.plugin_data.push(PluginData {
                name: plugin.clone(),
                version: version.clone(),
                configuration: config.pact_configuration.clone()
              });
            });
          }
          Ok(0)
        }
        Err(err) => {
          error!("{}", err);
          set_error_msg(err.to_string());
          Ok(6)
        }
      }
      None => Ok(3)
    }
  }).unwrap_or(1)
}

// TODO: This needs to setup rules/generators based on the content type
fn setup_core_matcher(interaction: &mut SynchronousHttp, part: InteractionPart, content_type: &ContentType, definition: &Value) {
  let part: &mut dyn HttpPart = match part {
    InteractionPart::Request => &mut interaction.request,
    InteractionPart::Response => &mut interaction.response
  };
  match definition {
    Value::String(s) => *part.body_mut() = OptionalBody::Present(Bytes::from(s.clone()), Some(content_type.clone()), None),
    Value::Object(ref o) => if o.contains_key("contents") {
      *part.body_mut() = body_from_json(&definition, "contents", &None);
    }
    _ => {}
  }
}

fn setup_contents(interaction: &mut SynchronousHttp, part: InteractionPart, content_type: &ContentType, definition: &Value) -> anyhow::Result<Option<(String, String, PluginConfiguration)>> {
  match find_content_matcher(&content_type) {
    Some(matcher) => {
      debug!("Found a matcher for '{}': {:?}", content_type, matcher);
      if matcher.is_core() {
        debug!("Matcher is from the core framework");
        setup_core_matcher(interaction, part, &content_type, definition);
        Ok(None)
      } else {
        debug!("Plugin matcher, will get the plugin to provide the part contents");
        match definition {
          Value::Object(attributes) => {
            let map = attributes.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let result = runtime.block_on(matcher.configure_interation(&content_type, map));
            match result {
              Ok((contents, plugin_config)) => {
                debug!("Interaction contents = {:?}", contents);
                debug!("Interaction plugin_config = {:?}", plugin_config);

                let part: &mut dyn HttpPart = match part {
                  InteractionPart::Request => &mut interaction.request,
                  InteractionPart::Response => &mut interaction.response
                };
                if let Some(contents) = contents.first() {
                  *part.body_mut() = contents.body.clone();
                  if !part.has_header("content-type") {
                    part.add_header("content-type", vec![content_type.to_string().as_str()]);
                  }
                  if let Some(rules) = &contents.rules {
                    part.matching_rules_mut().add_rules("body", rules.clone());
                  }
                  if let Some(generators) = &contents.generators {
                    part.generators_mut().add_generators(generators.clone());
                  }
                  if !contents.plugin_config.is_empty() {
                    interaction.plugin_config_mut().insert(matcher.plugin_name(), contents.plugin_config.interaction_configuration.clone());
                  }
                  *interaction.interaction_markup_mut() = InteractionMarkup {
                    markup: contents.interaction_markup.clone(),
                    markup_type: contents.interaction_markup_type.clone()
                  };
                }

                Ok(plugin_config.map(|config| (matcher.plugin_name(), matcher.plugin_version(), config)))
              }
              Err(err) => Err(anyhow!("Failed to call out to plugin - {}", err))
            }
          }
          _ => Err(anyhow!("{} is not a valid value for contents", definition))
        }
      }
    }
    None => {
      debug!("No matcher was found, will default to the core framework");
      setup_core_matcher(interaction, part, &content_type, definition);
      Ok(None)
    }
  }
}

#[cfg(test)]
mod tests {
  use std::ffi::CString;
  use std::ptr::null;

  use expectest::prelude::*;

  use crate::mock_server::handles::{InteractionHandle, InteractionPart, PactHandle};

  use super::pactffi_interaction_contents;

  #[test]
  fn pactffi_interaction_contents_with_invalid_content_type() {
    let pact_handle = PactHandle::new("Test", "Test");
    let i_handle = InteractionHandle::new(pact_handle, 0);
    expect!(pactffi_interaction_contents(i_handle, InteractionPart::Request, null(), null())).to(be_equal_to(1));

    let content_type = CString::new("not valid").unwrap();
    expect!(pactffi_interaction_contents(i_handle, InteractionPart::Request, content_type.as_ptr(), null())).to(be_equal_to(4));
  }

  #[test]
  fn pactffi_interaction_contents_with_invalid_contents() {
    let pact_handle = PactHandle::new("Test", "Test");
    let i_handle = InteractionHandle::new(pact_handle, 0);
    let content_type = CString::new("application/json").unwrap();
    expect!(pactffi_interaction_contents(i_handle, InteractionPart::Request, null(), null())).to(be_equal_to(1));

    let contents = CString::new("not valid").unwrap();
    expect!(pactffi_interaction_contents(i_handle, InteractionPart::Request, content_type.as_ptr(), contents.as_ptr())).to(be_equal_to(5));
  }
}
