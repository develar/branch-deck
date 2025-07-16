use opentelemetry::global;
use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::trace::Tracer;
use std::env;
use std::error::Error;
use tracing_subscriber::layer::SubscriberExt;

use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::SpanExporter;
use opentelemetry_sdk::trace::{BatchSpanProcessor, SdkTracerProvider};

/// Initialize telemetry with immediate OpenTelemetry setup using Tauri's runtime
pub fn init_telemetry(app_name: &str) {
  // Only initialize if no global subscriber is set yet
  if tracing::dispatcher::has_been_set() {
    return;
  }

  // Initialize LogTracer to convert log records to tracing events
  let _ = tracing_log::LogTracer::init();

  // Set up tracing subscriber with environment filter
  let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
    #[cfg(debug_assertions)]
    return tracing_subscriber::EnvFilter::new("debug,hyper=info,reqwest=info,tokenizers=off,candle=off,candle_core=off,candle_nn=off");
    #[cfg(not(debug_assertions))]
    return tracing_subscriber::EnvFilter::new("info,tokenizers=off,candle=off,candle_core=off,candle_nn=off");
  });

  // Initialize OpenTelemetry tracer if OTLP endpoint is configured
  let telemetry_layer = if env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok() {
    match init_opentelemetry(app_name) {
      Ok(tracer) => Some(tracing_opentelemetry::OpenTelemetryLayer::new(tracer)),
      Err(e) => {
        eprintln!("Failed to initialize OpenTelemetry: {e}");
        None
      }
    }
  } else {
    None
  };

  // Build the subscriber with all layers
  let subscriber = tracing_subscriber::registry().with(filter).with(tracing_subscriber::fmt::layer());

  if let Some(telemetry) = telemetry_layer {
    use tracing_subscriber::util::SubscriberInitExt;
    subscriber.with(telemetry).try_init().ok();
  } else {
    use tracing_subscriber::util::SubscriberInitExt;
    subscriber.try_init().ok();
  }
}

fn init_opentelemetry(service_name: &str) -> Result<Tracer, Box<dyn Error>> {
  // Configure the OpenTelemetry exporter for HTTP endpoint
  // Note: HTTP endpoint typically uses port 4318, gRPC uses 4317
  let endpoint = env::var("OTEL_EXPORTER_OTLP_ENDPOINT").unwrap_or_else(|_| "http://127.0.0.1:4318".to_string());

  // Create a reqwest client that will use the current Tokio runtime (Tauri's)
  // Since this function is called after Tauri setup, we're in Tauri's runtime context
  let client = reqwest::blocking::ClientBuilder::new().build()?;

  let exporter = SpanExporter::builder()
    .with_http()
    .with_endpoint(format!("{}/v1/traces", endpoint.trim_end_matches('/')))
    .with_http_client(client)
    .build()?;

  // Create a simple batch processor that will use the current runtime (Tauri's)
  let batch_processor = BatchSpanProcessor::builder(exporter).build();

  // Create the provider with the batch processor
  // Use default resource for now to avoid API complexity
  let provider = SdkTracerProvider::builder().with_span_processor(batch_processor).build();

  // Set as global provider
  let _ = global::set_tracer_provider(provider.clone());

  // Get a tracer
  let tracer = provider.tracer(service_name.to_string());

  Ok(tracer)
}
