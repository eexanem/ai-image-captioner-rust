// Web-based AI Image Captioner using Replicate API
// 
// Cargo.toml:
// [dependencies]
// axum = { version = "0.7", features = ["multipart"] }
// tokio = { version = "1", features = ["full"] }
// tower = "0.4"
// tower-http = { version = "0.5", features = ["fs", "cors"] }
// serde = { version = "1.0", features = ["derive"] }
// serde_json = "1.0"
// reqwest = { version = "0.11", features = ["json", "multipart"] }
// base64 = "0.22"
// image = "0.24"
// anyhow = "1.0"
// dotenvy = "0.15"

use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
struct AppState {
    api_key: String,
}

#[derive(Serialize, Deserialize)]
struct CaptionResponse {
    caption: String,
    model: String,
    processing_time_ms: u128,
}

async fn generate_caption(
    image_base64: String,
    api_key: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key={}",
        api_key
    );
    
    let payload = serde_json::json!({
        "contents": [{
            "parts": [
                {
                    "text": "Describe this image in detail. Provide a clear, descriptive caption."
                },
                {
                    "inline_data": {
                        "mime_type": "image/jpeg",
                        "data": image_base64
                    }
                }
            ]
        }]
    });
    
    println!("📤 Sending request to Google Gemini...");
    
    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await?;

    let status = response.status();
    let response_text = response.text().await?;
    
    println!("=== GEMINI RESPONSE ===");
    println!("Status: {}", status);
    println!("Body: {}", &response_text[..response_text.len().min(500)]);
    println!("=======================");

    if !status.is_success() {
        return Err(format!("API Error {}: {}", status, response_text).into());
    }

    let result: serde_json::Value = serde_json::from_str(&response_text)?;
    
    let caption = result["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .ok_or("No caption in response")?
        .to_string();
    
    println!("✅ Success! Caption: {}", caption);
    
    Ok(caption)
}

async fn upload_image(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<CaptionResponse>, StatusCode> {
    let start = std::time::Instant::now();

    while let Some(field) = multipart.next_field().await.unwrap() {
        let data = field.bytes().await.unwrap();

        let img = image::load_from_memory(&data)
            .map_err(|_| StatusCode::BAD_REQUEST)?;

        let mut jpeg_bytes = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut jpeg_bytes),
            image::ImageOutputFormat::Jpeg(85),
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let base64_img = general_purpose::STANDARD.encode(&jpeg_bytes);

        let caption = generate_caption(base64_img, &state.api_key)
            .await
            .map_err(|e| {
                eprintln!("Caption error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let elapsed = start.elapsed().as_millis();

        return Ok(Json(CaptionResponse {
            caption,
            model: "Google Gemini 1.5 Flash".to_string(),
            processing_time_ms: elapsed,
        }));
    }

    Err(StatusCode::BAD_REQUEST)
}

async fn index() -> Html<&'static str> {
    Html(
        r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>AI Image Captioner - Rust POC</title>
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }

        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
            padding: 20px;
        }

        .container {
            background: white;
            border-radius: 20px;
            box-shadow: 0 20px 60px rgba(0,0,0,0.3);
            max-width: 800px;
            width: 100%;
            padding: 40px;
        }

        h1 {
            color: #333;
            margin-bottom: 10px;
            font-size: 2em;
        }

        .subtitle {
            color: #666;
            margin-bottom: 30px;
            font-size: 0.9em;
        }

        .upload-area {
            border: 3px dashed #667eea;
            border-radius: 15px;
            padding: 60px 20px;
            text-align: center;
            cursor: pointer;
            transition: all 0.3s;
            background: #f8f9ff;
        }

        .upload-area:hover {
            border-color: #764ba2;
            background: #f0f2ff;
        }

        .upload-area.dragover {
            border-color: #764ba2;
            background: #e8ebff;
            transform: scale(1.02);
        }

        .upload-icon {
            font-size: 4em;
            margin-bottom: 20px;
        }

        .upload-text {
            color: #667eea;
            font-size: 1.2em;
            font-weight: 600;
            margin-bottom: 10px;
        }

        .upload-hint {
            color: #999;
            font-size: 0.9em;
        }

        input[type="file"] {
            display: none;
        }

        .preview-container {
            margin-top: 30px;
            display: none;
        }

        .preview-image {
            max-width: 100%;
            border-radius: 10px;
            margin-bottom: 20px;
            box-shadow: 0 4px 15px rgba(0,0,0,0.1);
        }

        .result {
            background: #f8f9ff;
            border-radius: 10px;
            padding: 20px;
            margin-top: 20px;
        }

        .result-label {
            color: #667eea;
            font-weight: 600;
            margin-bottom: 10px;
            font-size: 0.9em;
            text-transform: uppercase;
            letter-spacing: 1px;
        }

        .result-text {
            color: #333;
            font-size: 1.1em;
            line-height: 1.6;
        }

        .loading {
            text-align: center;
            padding: 40px;
            display: none;
        }

        .spinner {
            border: 4px solid #f3f3f3;
            border-top: 4px solid #667eea;
            border-radius: 50%;
            width: 50px;
            height: 50px;
            animation: spin 1s linear infinite;
            margin: 0 auto 20px;
        }

        @keyframes spin {
            0% { transform: rotate(0deg); }
            100% { transform: rotate(360deg); }
        }

        .meta-info {
            display: flex;
            justify-content: space-between;
            margin-top: 15px;
            padding-top: 15px;
            border-top: 1px solid #e0e0e0;
            font-size: 0.85em;
            color: #666;
        }

        .badge {
            display: inline-block;
            background: #667eea;
            color: white;
            padding: 4px 12px;
            border-radius: 20px;
            font-size: 0.8em;
            font-weight: 600;
        }

        .tech-stack {
            margin-top: 40px;
            padding-top: 30px;
            border-top: 2px solid #f0f0f0;
            text-align: center;
        }

        .tech-stack-title {
            color: #666;
            font-size: 0.85em;
            margin-bottom: 15px;
            text-transform: uppercase;
            letter-spacing: 1px;
        }

        .tech-badges {
            display: flex;
            gap: 10px;
            justify-content: center;
            flex-wrap: wrap;
        }

        .tech-badge {
            background: #f8f9ff;
            color: #667eea;
            padding: 8px 16px;
            border-radius: 20px;
            font-size: 0.85em;
            font-weight: 600;
            border: 2px solid #667eea;
        }

        .error {
            background: #fee;
            border: 2px solid #fcc;
            color: #c33;
            padding: 15px;
            border-radius: 10px;
            margin-top: 20px;
            display: none;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>🎨 AI Image Captioner</h1>
        <p class="subtitle">Rust + Google Gemini • Proof of Concept</p>

        <div class="upload-area" id="uploadArea">
            <div class="upload-icon">📸</div>
            <div class="upload-text">Click or drag image here</div>
            <div class="upload-hint">Supports JPG, PNG, WebP • Max 10MB</div>
            <input type="file" id="fileInput" accept="image/*">
        </div>

        <div class="loading" id="loading">
            <div class="spinner"></div>
            <p>Generating AI caption...</p>
        </div>

        <div class="error" id="error"></div>

        <div class="preview-container" id="previewContainer">
            <img id="previewImage" class="preview-image" alt="Preview">
            <div class="result">
                <div class="result-label">✨ AI Generated Caption</div>
                <div class="result-text" id="captionText"></div>
                <div class="meta-info">
                    <span>Model: <span class="badge" id="modelName">BLIP-2</span></span>
                    <span>Processing: <strong id="processingTime">--</strong>ms</span>
                </div>
            </div>
        </div>

        <div class="tech-stack">
            <div class="tech-stack-title">Built With</div>
            <div class="tech-badges">
                <span class="tech-badge">🦀 Rust</span>
                <span class="tech-badge">⚡ Axum</span>
                <span class="tech-badge">🤖 Google Gemini</span>
                <span class="tech-badge">🎯 Tokio</span>
            </div>
        </div>
    </div>

    <script>
        const uploadArea = document.getElementById('uploadArea');
        const fileInput = document.getElementById('fileInput');
        const loading = document.getElementById('loading');
        const previewContainer = document.getElementById('previewContainer');
        const previewImage = document.getElementById('previewImage');
        const captionText = document.getElementById('captionText');
        const modelName = document.getElementById('modelName');
        const processingTime = document.getElementById('processingTime');
        const errorDiv = document.getElementById('error');

        uploadArea.addEventListener('click', () => fileInput.click());

        uploadArea.addEventListener('dragover', (e) => {
            e.preventDefault();
            uploadArea.classList.add('dragover');
        });

        uploadArea.addEventListener('dragleave', () => {
            uploadArea.classList.remove('dragover');
        });

        uploadArea.addEventListener('drop', (e) => {
            e.preventDefault();
            uploadArea.classList.remove('dragover');
            const file = e.dataTransfer.files[0];
            if (file && file.type.startsWith('image/')) {
                handleFile(file);
            }
        });

        fileInput.addEventListener('change', (e) => {
            const file = e.target.files[0];
            if (file) {
                handleFile(file);
            }
        });

        async function handleFile(file) {
            const reader = new FileReader();
            reader.onload = (e) => {
                previewImage.src = e.target.result;
            };
            reader.readAsDataURL(file);

            uploadArea.style.display = 'none';
            loading.style.display = 'block';
            previewContainer.style.display = 'none';
            errorDiv.style.display = 'none';

            const formData = new FormData();
            formData.append('image', file);

            try {
                const response = await fetch('/upload', {
                    method: 'POST',
                    body: formData
                });

                if (!response.ok) {
                    throw new Error('Upload failed');
                }

                const result = await response.json();

                loading.style.display = 'none';
                previewContainer.style.display = 'block';
                captionText.textContent = result.caption;
                modelName.textContent = result.model.split(' ')[1];
                processingTime.textContent = result.processing_time_ms;

            } catch (error) {
                loading.style.display = 'none';
                uploadArea.style.display = 'block';
                errorDiv.textContent = 'Error: ' + error.message;
                errorDiv.style.display = 'block';
            }
        }
    </script>
</body>
</html>
        "#,
    )
}

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();
    
    let api_key = std::env::var("GEMINI_API_KEY")
        .expect("GEMINI_API_KEY must be set in .env file");

    let state = Arc::new(AppState { api_key });

    let app = Router::new()
        .route("/", get(index))
        .route("/upload", post(upload_image))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();

    println!("🚀 Server running on http://localhost:3000");
    println!("📸 Open in your browser to start captioning!");

    axum::serve(listener, app).await.unwrap();
}