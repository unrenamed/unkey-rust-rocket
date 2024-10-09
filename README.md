# Limit Your API Calls with Unkey Usage-Limited Keys

Integrate [Unkey](https://www.unkey.com/) to your API for managing call quotas. With usage-limited keys, this API enables you to control access while implementing automatic refill strategies—daily or monthly—ensuring seamless user experiences without exceeding limits.

## Use Cases

Quota limiting for APIs is essential for preventing abuse, managing costs, ensuring fair access, optimizing performance, and upholding security and compliance.

## Demo App Overview

This template showcases a generative AI REST API built with Rust and [Rocket](https://rocket.rs/) web framework. It demonstrates a secure method for storing created API keys using HTTP-only cookies. The `/generate-image` endpoint accepts a prompt message as payload, verifies the API key (decrementing remaining credits), and requests images from OpenAI. If the key becomes invalid, the user receives an error message indicating that the quota has been exceeded.

## Quickstart

### Create a root key

1. Go to [/settings/root-keys](https://app.unkey.com/settings/root-key) and click on the "Create New Root Key" button.
2. Enter a name for the key.
3. Select the following workspace permissions: `create_key`, `read_key`, `encrypt_key` and `decrypt_key`.
4. Click "Create".

### Create your API

1. Go to [https://app.unkey.com/apis](https://app.unkey.com/apis) and click on the "Create New API" button.
2. Give it a name.
3. Click "Create".

### Create yout OpenAI API key

1. Go to the [https://platform.openai.com/](https://platform.openai.com/) and create an account or log in.
2. Navigate to the API section in your dashboard.
3. Click on “Create API Key” or “Generate API Key.”
4. Copy and securely store the generated key.

### Set up the example

1. Clone the repository to your local machine:

```bash
git clone git@github.com:unrenamed/unkey-rust-rocket
cd unkey-rust-rocket
```

2. Create a `.env` file in the root directory and populate it with the following environment variables:

```env
OPENAI_API_KEY=your-openai-api-key
UNKEY_ROOT_KEY=your-unkey-root-key
UNKEY_API_ID=your-unkey-api-id
```

Ensure you replace `your-*` with your actual Unkey credentials.

4. Start the server:

```bash
cargo run
```

The server will start and listen on the port specified in the `.env` file (default is `8000`).

5. Use `/authorize` route to generate a new API key which will be saved to cookies:

```bash
  curl -X POST http://localhost:8000/authorize
```

6. Use `/me` route to ensure you've successfully authorized:

```bash
   curl http://localhost:8000/me
```

7. Send a prompt message to `/generate-image` route to receive a URL to the generated image:

```bash
  curl -X POST http://localhost:8000/generate_image \
    -H "Content-Type: application/json" \
    -d '{"prompt": "A sunset over a mountain range"}'
```
