//! ## Task Description
//!
//! The goal is to develop a backend service for shortening URLs using CQRS
//! (Command Query Responsibility Segregation) and ES (Event Sourcing)
//! approaches. The service should support the following features:
//!
//! ## Functional Requirements
//!
//! ### Creating a short link with a random slug
//!
//! The user sends a long URL, and the service returns a shortened URL with a
//! random slug.
//!
//! ### Creating a short link with a predefined slug
//!
//! The user sends a long URL along with a predefined slug, and the service
//! checks if the slug is unique. If it is unique, the service creates the short
//! link.
//!
//! ### Counting the number of redirects for the link
//!
//! - Every time a user accesses the short link, the click count should
//!   increment.
//! - The click count can be retrieved via an API.
//!
//! ### CQRS+ES Architecture
//!
//! CQRS: Commands (creating links, updating click count) are separated from
//! queries (retrieving link information).
//!
//! Event Sourcing: All state changes (link creation, click count update) must be
//! recorded as events, which can be replayed to reconstruct the system's state.
//!
//! ### Technical Requirements
//!
//! - The service must be built using CQRS and Event Sourcing approaches.
//! - The service must be possible to run in Rust Playground (so no database like
//!   Postgres is allowed)
//! - Public API already written for this task must not be changed (any change to
//!   the public API items must be considered as breaking change).

#![allow(unused_variables, dead_code)]

use std::collections::HashMap;
use rand::distr::Alphanumeric;
use rand::{Rng, thread_rng};

/// All possible errors of the [`UrlShortenerService`].
#[derive(Debug, PartialEq)]
pub enum ShortenerError {
    /// This error occurs when an invalid [`Url`] is provided for shortening.
    InvalidUrl,

    /// This error occurs when an attempt is made to use a slug (custom alias)
    /// that already exists.
    SlugAlreadyInUse,

    /// This error occurs when the provided [`Slug`] does not map to any existing
    /// short link.
    SlugNotFound,
}

/// Represents the different types of events that can occur within the
/// [`UrlShortenerService`].
///
/// Using event sourcing, each change or action taken is logged as an event.
/// This allows the current state to be reconstructed by replaying events.
enum Event {
    /// Event indicating that a new short link has been created.
    ///
    /// Contains the [`Slug`] and the original [`Url`] for the newly created short link.
    LinkCreated {
        /// The unique identifier for the short link.
        slug: Slug,
        /// The original URL that the short link points to.
        url: Url,
    },

    /// Event indicating that a redirect action has occurred for a short link.
    ///
    /// Contains the [`Slug`] of the short link that was used in the redirect.
    LinkRedirected {
        /// The unique identifier for the short link that was used in the redirect.
        slug: Slug,
    },
}

/// A unique string (or alias) that represents the shortened version of the
/// URL.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Slug(pub String);

/// The original URL that the short link points to.
#[derive(Clone, Debug, PartialEq)]
pub struct Url(pub String);

/// Shortened URL representation.
#[derive(Debug, Clone, PartialEq)]
pub struct ShortLink {
    /// A unique string (or alias) that represents the shortened version of the
    /// URL.
    pub slug: Slug,

    /// The original URL that the short link points to.
    pub url: Url,
}

/// Statistics of the [`ShortLink`].
#[derive(Debug, Clone, PartialEq)]
pub struct Stats {
    /// [`ShortLink`] to which this [`Stats`] are related.
    pub link: ShortLink,

    /// Count of redirects of the [`ShortLink`].
    pub redirects: u64,
}

/// Commands for CQRS.
pub mod commands {
    use super::{ShortLink, ShortenerError, Slug, Url};

    /// Trait for command handlers.
    pub trait CommandHandler {
        /// Creates a new short link. It accepts the original url and an
        /// optional [`Slug`]. If a [`Slug`] is not provided, the service will generate
        /// one. Returns the newly created [`ShortLink`].
        ///
        /// ## Errors
        ///
        /// See [`ShortenerError`].
        fn handle_create_short_link(
            &mut self,
            url: Url,
            slug: Option<Slug>,
        ) -> Result<ShortLink, ShortenerError>;

        /// Processes a redirection by [`Slug`], returning the associated
        /// [`ShortLink`] or a [`ShortenerError`].
        fn handle_redirect(
            &mut self,
            slug: Slug,
        ) -> Result<ShortLink, ShortenerError>;

        /// Updates the [`Url`] of an existing [`ShortLink`] using a provided [`Slug`].
        ///
        /// This function changes the destination URL associated with a given slug.
        /// If the slug exists, the URL is updated and the function returns the updated
        /// [`ShortLink`]. If the slug does not exist, an error is returned.
        ///
        /// ## Errors
        ///
        /// Returns a [`ShortenerError::SlugNotFound`] if the provided slug does not map
        /// to any existing short link.
        fn handle_change_short_link(
            &mut self,
            slug: Slug,
            new_url: Url,
        ) -> Result<ShortLink, ShortenerError>;
    }
}

/// Queries for CQRS
pub mod queries {
    use super::{ShortenerError, Slug, Stats};

    /// Trait for query handlers.
    pub trait QueryHandler {
        /// Returns the [`Stats`] for a specific [`ShortLink`], such as the
        /// number of redirects (clicks).
        ///
        /// [`ShortLink`]: super::ShortLink
        fn get_stats(&self, slug: Slug) -> Result<Stats, ShortenerError>;
    }
}

/// CQRS and Event Sourcing-based service implementation
pub struct UrlShortenerService {
    events: Vec<Event>,
    links: HashMap<Slug, Url>,
    click_counts: HashMap<Slug, u64>,
}

impl UrlShortenerService {
    /// Creates a new instance of the service
    pub fn new() -> Self {
        Self {
            events: vec![],
            links: HashMap::new(),
            click_counts: HashMap::new(),
        }
    }

    /// Generates a random slug using a UUID.
    ///
    /// This function creates a new UUID (Universally Unique Identifier) and extracts
    /// the first part before the first hyphen. The generated slug string is wrapped
    /// in a `Slug` struct to represent the generated slug.
    ///
    /// # Returns
    /// A `Slug` struct containing a randomly generated slug string.
    fn generate_random_slug() -> Slug {
        let slug: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(8)
            .map(char::from) // Convert to char
            .collect(); // Collect into a String
        Slug(slug)
    }

    /// Creates a LinkCreated event and adds it to the list of events
    fn add_link_created_event(&mut self, slug: Slug, url: Url) {
        self.events.push(Event::LinkCreated {
            slug: slug.clone(),
            url: url.clone(),
        });
        self.links.insert(slug, url);
    }

    /// Creates a LinkRedirected event and adds it to the list of events
    fn add_link_redirected_event(&mut self, slug: &Slug) {
        self.events.push(Event::LinkRedirected { slug: slug.clone() });
        *self.click_counts.entry(slug.clone()).or_insert(0) += 1;
    }
}

impl commands::CommandHandler for UrlShortenerService {
    fn handle_create_short_link(
        &mut self,
        url: Url,
        slug: Option<Slug>,
    ) -> Result<ShortLink, ShortenerError> {
        let slug = match slug {
            Some(custom_slug) => {
                if self.links.contains_key(&custom_slug) {
                    return Err(ShortenerError::SlugAlreadyInUse)
                }
                custom_slug
            }
            None => {
                let mut generated_slug = Self::generate_random_slug();
                while self.links.contains_key(&generated_slug) {
                    generated_slug = Self::generate_random_slug();
                }
                generated_slug
            }
        };

        self.add_link_created_event(slug.clone(), url.clone());

        Ok(ShortLink { slug, url})
    }

    fn handle_redirect(
        &mut self,
        slug: Slug,
    ) -> Result<ShortLink, ShortenerError> {
        let url = self.links.get(&slug).ok_or(ShortenerError::SlugNotFound)?.clone();

        self.add_link_redirected_event(&slug);

        Ok(ShortLink { slug, url })
    }

    fn handle_change_short_link(
        &mut self,
        slug: Slug,
        new_url: Url,
    ) -> Result<ShortLink, ShortenerError> {
        if !self.links.contains_key(&slug) {
            return Err(ShortenerError::SlugNotFound);
        }

        self.add_link_created_event(slug.clone(), new_url.clone());

        Ok(ShortLink { slug, url: new_url })
    }
}

impl queries::QueryHandler for UrlShortenerService {
    fn get_stats(&self, slug: Slug) -> Result<Stats, ShortenerError> {
        let url = self.links.get(&slug).ok_or(ShortenerError::SlugNotFound)?.clone();
        let redirects = *self.click_counts.get(&slug).unwrap_or(&0);

        Ok(Stats {
            link: ShortLink { slug, url },
            redirects
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::CommandHandler;
    use crate::queries::QueryHandler;

    #[test]
    fn test_random_slug_generation_creates_unique_link() {
        let mut service = UrlShortenerService::new();
        let url = Url("https://www.amazon.com/Redragon-S101-Keyboard-Ergonomic-Programmable/dp/B00NLZUM36/ref=sr_1_1?_encoding=UTF8".to_string());

        let result = service.handle_create_short_link(url.clone(), None);
        assert!(result.is_ok());

        let short_link = result.unwrap();
        assert_eq!(short_link.url, url);
        assert_eq!(service.links.len(), 1);
    }

    #[test]
    fn test_custom_slug_allows_exact_alias_creation() {
        let mut service = UrlShortenerService::new();
        let url = Url("https://www.amazon.com/Redragon-S101-Keyboard-Ergonomic-Programmable/dp/B00NLZUM36/ref=sr_1_1?_encoding=UTF8".to_string());
        let slug = Slug("my_slug".to_string());

        let result = service.handle_create_short_link(url.clone(), Some(slug.clone()));
        assert!(result.is_ok());

        let short_link = result.unwrap();
        assert_eq!(short_link.slug, slug);
        assert_eq!(short_link.url, url);
        assert_eq!(service.links.len(), 1);
    }

    #[test]
    fn test_duplicate_slug_creation_should_fail() {
        let mut service = UrlShortenerService::new();
        let url1 = Url("https://www.amazon.com/Redragon-S101-Keyboard-Ergonomic-Programmable/dp/B00NLZUM36/ref=sr_1_1?_encoding=UTF8".to_string());
        let slug = Slug("my_slug".to_string());

        service.handle_create_short_link(url1.clone(), Some(slug.clone())).unwrap();

        let url2 = Url("https://www.amazon.com/Redragon-Keyboard-Wireless-Independent-Multimedia/dp/B0CTMMN857/ref=pd_ci_mcx_pspc_dp_2_i_2?pd_rd_w=J8fu0".to_string());
        let result = service.handle_create_short_link(url2, Some(slug));
        assert!(matches!(result, Err(ShortenerError::SlugAlreadyInUse)));
    }

    #[test]
    fn test_redirect_increases_click_count_for_existing_slug() {
        let mut service = UrlShortenerService::new();
        let url = Url("https://www.amazon.com/Redragon-S101-Keyboard-Ergonomic-Programmable/dp/B00NLZUM36/ref=sr_1_1?_encoding=UTF".to_string());
        let slug = Slug("my_slug".to_string());

        service.handle_create_short_link(url.clone(), Some(slug.clone())).unwrap();

        let result = service.handle_redirect(slug.clone());
        assert!(result.is_ok());

        let short_link = result.unwrap();
        assert_eq!(short_link.url, url);
        assert_eq!(short_link.slug, slug);
        assert_eq!(service.click_counts.get(&slug), Some(&1));
    }

    #[test]
    fn test_redirect_fails_for_nonexistent_slug() {
        let mut service = UrlShortenerService::new();
        let slug = Slug("slug_does_not_exist".to_string());

        let result = service.handle_redirect(slug);
        assert!(matches!(result, Err(ShortenerError::SlugNotFound)));
    }

    #[test]
    fn test_stats_retrieval_reflects_actual_clicks_for_valid_slug() {
        let mut service = UrlShortenerService::new();
        let url = Url("https://www.amazon.com/Redragon-S101-Keyboard-Ergonomic-Programmable/dp/B00NLZUM36/ref=sr_1_1?_encoding=UTF".to_string());
        let slug = Slug("my_slug".to_string());

        service.handle_create_short_link(url.clone(), Some(slug.clone())).unwrap();
        service.handle_redirect(slug.clone()).unwrap();
        service.handle_redirect(slug.clone()).unwrap();

        let result = service.get_stats(slug.clone());
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.link.url, url);
        assert_eq!(stats.redirects, 2);
    }

    #[test]
    fn test_stats_retrieval_fails_gracefully_for_missing_slug() {
        let service = UrlShortenerService::new();
        let slug = Slug("slug_does_not_exist".to_string());

        let result = service.get_stats(slug);
        assert!(matches!(result, Err(ShortenerError::SlugNotFound)));
    }

    #[test]
    fn test_change_short_link_success() {
        let mut service = UrlShortenerService::new();
        let original_url = Url("https://www.amazon.com/Redragon-S101-Keyboard-Ergonomic-Programmable/dp/B00NLZUM36/ref=sr_1_1?_encoding=UTF8".into());
        let new_url = Url("https://www.amazon.com/Redragon-Keyboard-Wireless-Independent-Multimedia/dp/B0CTMMN857/ref=pd_ci_mcx_pspc_dp_2_i_2?pd_rd_w=J8fu0".into());
        let slug = Slug("my_slug".into());

        // Create the initial short link
        service.handle_create_short_link(original_url.clone(), Some(slug.clone())).unwrap();

        // Change the short link's destination URL
        let result = service.handle_change_short_link(slug.clone(), new_url.clone());

        assert!(result.is_ok());
        let updated_short_link = result.unwrap();
        assert_eq!(updated_short_link.url, new_url);
        assert_eq!(updated_short_link.slug, slug);

        // Verify that the internal state reflects the change
        let stats = service.get_stats(slug.clone()).unwrap();
        assert_eq!(stats.link.url, new_url);
    }

    #[test]
    fn test_change_short_link_slug_not_found() {
        let mut service = UrlShortenerService::new();
        let new_url = Url("https://www.amazon.com/Redragon-Keyboard-Wireless-Independent-Multimedia/dp/B0CTMMN857/ref=pd_ci_mcx_pspc_dp_2_i_2?pd_rd_w=J8fu0".into());
        let slug = Slug("slug_does_not_exist".into());

        // Attempt to change a non-existent short link
        let result = service.handle_change_short_link(slug.clone(), new_url.clone());

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ShortenerError::SlugNotFound);
    }
}