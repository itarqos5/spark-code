use crate::model::Provider;

const MAX_MODELS: usize = 100;
const MAX_MODEL_NAME_BYTES: usize = 256;

/// Model lists belong to the provider that returned them, not the active UI selection.
#[derive(Default)]
pub struct ModelCatalog {
    codex: Vec<String>,
    claude: Vec<String>,
}

impl ModelCatalog {
    pub fn get(&self, provider: Provider) -> &[String] {
        match provider {
            Provider::Codex => &self.codex,
            Provider::Claude => &self.claude,
        }
    }

    /// Replace one provider's list, retaining the first 100 unique, bounded names.
    pub fn set(&mut self, provider: Provider, models: Vec<String>) {
        let mut bounded = Vec::with_capacity(models.len().min(MAX_MODELS));
        for model in models {
            // Skip oversized names rather than truncate a UTF-8 model identifier.
            if model.len() > MAX_MODEL_NAME_BYTES || bounded.contains(&model) {
                continue;
            }
            bounded.push(model);
            if bounded.len() == MAX_MODELS {
                break;
            }
        }
        match provider {
            Provider::Codex => self.codex = bounded,
            Provider::Claude => self.claude = bounded,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(models: &[&str]) -> Vec<String> {
        models.iter().map(|model| (*model).to_owned()).collect()
    }

    #[test]
    fn default_catalog_is_empty_for_both_providers() {
        let catalog = ModelCatalog::default();
        for provider in Provider::ALL {
            assert!(catalog.get(provider).is_empty());
        }
    }

    #[test]
    fn updating_one_provider_preserves_the_other() {
        let mut catalog = ModelCatalog::default();
        catalog.set(Provider::Codex, names(&["codex-old"]));
        catalog.set(Provider::Claude, names(&["claude-old"]));

        catalog.set(Provider::Codex, names(&["codex-new"]));
        assert_eq!(catalog.get(Provider::Codex), ["codex-new"]);
        assert_eq!(catalog.get(Provider::Claude), ["claude-old"]);

        catalog.set(Provider::Claude, names(&["claude-new"]));
        assert_eq!(catalog.get(Provider::Codex), ["codex-new"]);
        assert_eq!(catalog.get(Provider::Claude), ["claude-new"]);
    }

    #[test]
    fn empty_refresh_clears_only_its_provider() {
        for cleared in Provider::ALL {
            let mut catalog = ModelCatalog::default();
            catalog.set(Provider::Codex, names(&["codex-model"]));
            catalog.set(Provider::Claude, names(&["claude-model"]));
            catalog.set(cleared, Vec::new());

            assert!(catalog.get(cleared).is_empty());
            match cleared {
                Provider::Codex => {
                    assert_eq!(catalog.get(Provider::Claude), ["claude-model"]);
                }
                Provider::Claude => {
                    assert_eq!(catalog.get(Provider::Codex), ["codex-model"]);
                }
            }
        }
    }

    #[test]
    fn late_response_updates_request_provider_after_selection_changes() {
        for request_provider in Provider::ALL {
            let selected_provider = match request_provider {
                Provider::Codex => Provider::Claude,
                Provider::Claude => Provider::Codex,
            };
            let mut catalog = ModelCatalog::default();
            catalog.set(selected_provider, names(&["current-model"]));

            // A delayed request retains its originating provider after the UI switches.
            catalog.set(request_provider, names(&["delayed-model"]));

            assert_eq!(catalog.get(request_provider), ["delayed-model"]);
            assert_eq!(catalog.get(selected_provider), ["current-model"]);
        }
    }

    #[test]
    fn deduplicates_without_reordering() {
        let mut catalog = ModelCatalog::default();
        catalog.set(
            Provider::Codex,
            names(&["second", "first", "second", "third", "first"]),
        );
        assert_eq!(catalog.get(Provider::Codex), ["second", "first", "third"]);
    }

    #[test]
    fn skips_oversized_names_without_truncating_utf8() {
        let ascii_at_limit = "a".repeat(MAX_MODEL_NAME_BYTES);
        let unicode_at_limit = "é".repeat(MAX_MODEL_NAME_BYTES / 2);
        let mut catalog = ModelCatalog::default();
        catalog.set(
            Provider::Codex,
            vec![
                "a".repeat(MAX_MODEL_NAME_BYTES + 1),
                ascii_at_limit.clone(),
                "é".repeat(MAX_MODEL_NAME_BYTES / 2 + 1),
                unicode_at_limit.clone(),
                "valid-after-oversized".to_owned(),
            ],
        );
        assert_eq!(
            catalog.get(Provider::Codex),
            [
                ascii_at_limit,
                unicode_at_limit,
                "valid-after-oversized".to_owned()
            ]
        );
    }

    #[test]
    fn caps_each_provider_at_first_100_unique_valid_names() {
        let mut catalog = ModelCatalog::default();
        for provider in Provider::ALL {
            let mut models = Vec::new();
            for index in 0..MAX_MODELS + 10 {
                models.push("x".repeat(MAX_MODEL_NAME_BYTES + 1));
                models.push(format!("{}-{index}", provider.cli()));
                models.push(format!("{}-{index}", provider.cli()));
            }
            catalog.set(provider, models);
        }
        for provider in Provider::ALL {
            let expected: Vec<_> = (0..MAX_MODELS)
                .map(|index| format!("{}-{index}", provider.cli()))
                .collect();
            assert_eq!(catalog.get(provider), expected);
        }
    }
}
