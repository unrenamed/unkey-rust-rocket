use anyhow::{Context, Result};
use reqwest::Client;
use rocket::http::{Cookie, CookieJar, Status};
use rocket::response::Redirect;
use rocket::serde::json;
use rocket::serde::json::{Json, Value};
use rocket::serde::{Deserialize, Serialize};
use rocket::{get, launch, post, routes, uri};
use unkey::models::{CreateKeyRequest, Refill, RefillInterval, VerifyKeyRequest};
use unkey::Client as UnkeyClient;

use std::env;

// Lazy initialization of environment variables
lazy_static::lazy_static! {
    static ref UNKEY_ROOT_KEY: String = get_env("UNKEY_ROOT_KEY", "");
    static ref UNKEY_API_ID: String = get_env("UNKEY_API_ID", "");
    static ref OPENAI_API_KEY: String = get_env("OPENAI_API_KEY", "");
}

/// Helper function for reading environment variables with default fallback
fn get_env(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Define return type for image generation responses
type GenerateImageReturnType = (Status, Json<Value>);

/// Struct for data returned upon key creation
#[derive(Deserialize, Serialize, Debug)]
#[serde(crate = "rocket::serde")]
struct KeyCreateData {
    key: String,
    key_id: String,
}

/// Struct for data returned upon key verification
#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct KeyVerifyData {
    valid: bool,
    remaining: Option<usize>,
}

/// Request struct for image generation with OpenAI
#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
struct GenerateImageRequest {
    prompt: String,
}

/// Response struct for OpenAI's image generation
#[derive(Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct OpenAIResponse {
    data: Vec<ImageData>,
}

/// Struct to hold the URL of the generated image
#[derive(Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct ImageData {
    url: String,
}

// Launch the Rocket application
#[launch]
async fn rocket() -> _ {
    // Load environment variables from .env file
    dotenv::dotenv().ok();
    // Mount routes for the application
    rocket::build().mount("/", routes![me, authorize, generate_image])
}

/// Endpoint to retrieve the current user's key information
#[get("/me")]
async fn me(jar: &CookieJar<'_>) -> Result<Json<KeyCreateData>, Status> {
    jar.get("unkey")
        .and_then(|cookie| json::from_str(cookie.value()).ok())
        .map_or_else(
            || Err(Status::Unauthorized), // Return 401 if no key found
            |unkey_data: KeyCreateData| Ok(Json(unkey_data)),
        )
}

/// Endpoint to authorize a user and create a new API key
#[post("/authorize")]
async fn authorize(jar: &CookieJar<'_>) -> Result<Redirect, Status> {
    if let Some(data) = create_key().await {
        let value = json::to_string(&data).unwrap();
        let cookie = Cookie::build(("unkey", value)).http_only(true).build(); // Create HTTP-only cookie
        jar.add(cookie);
        Ok(Redirect::to(uri!(me()))) // Redirect to the "me" endpoint
    } else {
        Err(Status::Unauthorized) // Return 401 if key creation fails
    }
}

/// Endpoint to generate an image based on a provided prompt
#[post("/generate_image", format = "json", data = "<payload>")]
async fn generate_image(
    jar: &CookieJar<'_>,
    payload: Json<GenerateImageRequest>, // Request payload containing prompt
) -> Result<GenerateImageReturnType, GenerateImageReturnType> {
    // Helper function to respond with an error
    fn error_response(status: Status, message: &str) -> (Status, Json<Value>) {
        (status, Json(json::json!({ "error": message })))
    }

    // Check for the presence of the "unkey" cookie
    let cookie = match jar.get("unkey") {
        Some(cookie) => cookie,
        None => {
            return Ok(error_response(
                Status::Unauthorized,
                "Unauthorized: Missing API key in cookies.",
            ));
        }
    };

    let value = cookie.value();
    let unkey_data: KeyCreateData = match json::from_str(value) {
        // Deserialize the key data
        Ok(data) => data,
        Err(_) => {
            return Ok(error_response(
                Status::BadRequest,
                "Invalid API key format in cookies.",
            ));
        }
    };

    // Verify the key
    let key = match verify_key(&unkey_data.key).await {
        Some(key) if key.valid => key,
        _ => {
            return Ok(error_response(
                Status::BadRequest,
                "Invalid API key: Quota exceeded or invalid key.",
            ));
        }
    };

    // Call OpenAI API to generate the image
    match request_image_from_openai(&payload.prompt).await {
        Ok(image_url) => {
            let response = json::json!({
                "image_url": image_url,
                "remaining_calls": key.remaining
            });
            Ok((Status::Ok, Json(response)))
        }
        Err(e) => {
            eprintln!("Error generating image: {:?}", e);
            Ok(error_response(
                Status::InternalServerError,
                "Internal server error: Unable to generate the image.",
            ))
        }
    }
}

/// Helper function to request an image from OpenAI's API
async fn request_image_from_openai(prompt: &str) -> Result<String> {
    let client = Client::new(); // Create a new HTTP client
    let body = json::json!({
        "prompt": prompt,
        "n": 1, // Number of images to generate
        "size": "1024x1024",
        "response_format": "url"
    });

    // Send request to OpenAI API
    let response: OpenAIResponse = client
        .post("https://api.openai.com/v1/images/generations")
        .bearer_auth(OPENAI_API_KEY.as_str())
        .json(&body)
        .send()
        .await
        .context("Failed to send request to OpenAI")? // Handle potential request errors
        .json()
        .await
        .context("Failed to deserialize response from OpenAI")?; // Handle potential deserialization errors

    response
        .data
        .first()
        .map(|image| image.url.clone())
        .context("No image returned by OpenAI") // Handle case where no image is returned
}

/// Function to create a new API key using Unkey service
async fn create_key() -> Option<KeyCreateData> {
    let unkey_client = UnkeyClient::new(UNKEY_ROOT_KEY.as_str());
    let req = CreateKeyRequest::new(UNKEY_API_ID.as_str())
        .set_remaining(10)
        .set_refill(Refill::new(10, RefillInterval::Daily))
        .set_owner_id("superuser");

    unkey_client
        .create_key(req)
        .await
        .ok()
        .map(|res| KeyCreateData {
            key: res.key,
            key_id: res.key_id,
        })
}

/// Function to verify an API key using Unkey service
async fn verify_key(key: &str) -> Option<KeyVerifyData> {
    let unkey_client = UnkeyClient::new(UNKEY_ROOT_KEY.as_str());
    let req = VerifyKeyRequest::new(key, UNKEY_API_ID.as_str());

    unkey_client
        .verify_key(req)
        .await
        .ok()
        .map(|res| KeyVerifyData {
            valid: res.valid,
            remaining: res.remaining,
        })
}
