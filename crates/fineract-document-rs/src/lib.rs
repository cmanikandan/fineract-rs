//! `fineract-document-rs` — Document Management, Content Repository (Filesystem/S3), and Entity Image Storage.
//!
//! Ported from:
//! - `org.apache.fineract.infrastructure.contentstore.data.ContentStoreType` (`StorageType`)
//! - `org.apache.fineract.infrastructure.contentstore.policy.{TraversalContentPolicy, WhitelistContentPolicy, MimeContentPolicy}`
//! - `org.apache.fineract.infrastructure.contentstore.processor.{DataUrlDecoderContentProcessor, DataUrlEncoderContentProcessor, Base64DecoderContentProcessor, Base64EncoderContentProcessor}`
//! - `org.apache.fineract.infrastructure.contentstore.service.{ContentStoreService, FileContentStoreService, S3ContentStoreService}`
//! - `org.apache.fineract.infrastructure.contentstore.util.{DefaultContentPathSanitizer, DefaultContentPathRandomizer}`
//! - `org.apache.fineract.infrastructure.documentmanagement.api.{DocumentApiConstants, ImageApiConstants, FileUploadValidator, DocumentApiResource, ImagesApiResource}`
//! - `org.apache.fineract.infrastructure.documentmanagement.domain.{Document, DocumentRepository, Image, ImageRepository}`
//! - `org.apache.fineract.infrastructure.documentmanagement.data.{DocumentData, DocumentContent, DocumentCreateRequest, DocumentUpdateRequest, DocumentDeleteRequest, ImageCreateRequest, ImageDeleteRequest}`
//! - `org.apache.fineract.infrastructure.documentmanagement.service.{DocumentReadPlatformServiceImpl, DocumentWritePlatformServiceImpl, ImageReadPlatformServiceImpl, ImageWritePlatformServiceImpl}`
//! - `org.apache.fineract.infrastructure.documentmanagement.handler.{DocumentCreateCommandHandler, DocumentUpdateCommandHandler, DocumentDeleteCommandHandler, ImageCreateCommandHandler, ImageDeleteCommandHandler}`

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sqlx::{AnyPool, Row};
use uuid::Uuid;

use fineract_command_rs::{
    CommandHandler, CommandPipeline, CommandProcessingResult, CommandProcessingResultBuilder,
};
use fineract_core_rs::{DialectSql, ExecutionContext, JsonCommand, SqlDialect};
use fineract_validation_rs::{
    ApiErrorArg, ApiParameterError, DataValidatorBuilder, FineractError,
    PlatformApiDataValidationException,
};

// =============================================================================
// CONSTANTS (DocumentApiConstants & ImageApiConstants)
// =============================================================================

pub const DOCUMENT_API_PARAM_ENTITY_TYPE: &str = "entityType";
pub const DOCUMENT_API_PARAM_ENTITY_ID: &str = "entityId";
pub const DOCUMENT_API_PARAM_DOCUMENT_ID: &str = "documentId";
pub const DOCUMENT_API_PARAM_FILE: &str = "file";
pub const DOCUMENT_API_PARAM_NAME: &str = "name";
pub const DOCUMENT_API_PARAM_DESCRIPTION: &str = "description";

pub const IMAGE_API_PARAM_MAX_WIDTH: &str = "maxWidth";
pub const IMAGE_API_PARAM_MAX_HEIGHT: &str = "maxHeight";
pub const IMAGE_API_PARAM_OUTPUT: &str = "output";
pub const IMAGE_API_VALUE_OUTPUT_INLINE_OCTET: &str = "inline_octet";
pub const IMAGE_API_VALUE_IMAGE_TYPE_JPG: &str = "jpg";
pub const IMAGE_API_VALUE_ENCODING_BASE64: &str = "base64";

pub const DOCUMENTS_STORE_PREFIX: &str = "documents";
pub const IMAGES_STORE_PREFIX: &str = "images";
pub const DEFAULT_IMAGE_ENTITY_TYPE: &str = "clients";
pub const DEFAULT_IMAGE_EXTENSION: &str = ".jpg";
pub const APPLICATION_OCTET_STREAM: &str = "application/octet-stream";

// =============================================================================
// STORAGE TYPE (StorageType.java / ContentStoreType.java)
// =============================================================================

/// Content repository storage backend discriminant stored in `m_document.storage_type_enum`
/// and `m_image.storage_type_enum`.
///
/// Ported from `org.apache.fineract.infrastructure.contentstore.data.ContentStoreType`
/// (`StorageType.java`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(i32)]
pub enum StorageType {
    #[default]
    FileSystem = 1,
    S3 = 2,
}

/// Alias matching `org.apache.fineract.infrastructure.contentstore.data.ContentStoreType`.
pub type ContentStoreType = StorageType;

impl StorageType {
    /// Java-compatible constant alias for `StorageType::FileSystem`.
    pub const FILE_SYSTEM: Self = Self::FileSystem;
    /// Java-compatible constant alias for `StorageType::S3`.
    pub const S3: Self = Self::S3;

    /// Returns the integer discriminant (`1` for `FileSystem`, `2` for `S3`) matching `getValue()`.
    pub const fn value(self) -> i32 {
        self as i32
    }

    /// Returns the `i32` representation of the storage type.
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    /// Returns the `SMALLINT` (`i16`) database representation for `storage_type_enum`.
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    /// Parses an `i32` discriminant into [`StorageType`].
    pub const fn from_int(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::FileSystem),
            2 => Some(Self::S3),
            _ => None,
        }
    }

    /// Parses an `i16` (`SMALLINT`) discriminant into [`StorageType`].
    pub const fn from_i16(value: i16) -> Option<Self> {
        match value {
            1 => Some(Self::FileSystem),
            2 => Some(Self::S3),
            _ => None,
        }
    }

    /// Returns the canonical string code for the storage backend.
    pub const fn code(self) -> &'static str {
        match self {
            Self::FileSystem => "FILE_SYSTEM",
            Self::S3 => "S3",
        }
    }

    /// Returns `true` if `self == StorageType::FileSystem`.
    pub const fn is_file_system(self) -> bool {
        matches!(self, Self::FileSystem)
    }

    /// Returns `true` if `self == StorageType::S3`.
    pub const fn is_s3(self) -> bool {
        matches!(self, Self::S3)
    }
}

impl fmt::Display for StorageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

// =============================================================================
// DOCUMENT PARENT ENTITY TYPE
// =============================================================================

/// Supported parent entities to which documents and images can be attached in Fineract.
///
/// Ported from `DocumentApiResource` (`clients`, `staff`, `loans`, `savings`,
/// `client_identifiers`, `groups`, `import`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentParentEntityType {
    Clients,
    Staff,
    Loans,
    Savings,
    ClientIdentifiers,
    Groups,
    Import,
}

impl DocumentParentEntityType {
    /// Returns the canonical URL / database `parent_entity_type` segment.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Clients => "clients",
            Self::Staff => "staff",
            Self::Loans => "loans",
            Self::Savings => "savings",
            Self::ClientIdentifiers => "client_identifiers",
            Self::Groups => "groups",
            Self::Import => "import",
        }
    }

    /// Parses an entity type path parameter into [`DocumentParentEntityType`].
    pub fn parse(raw: &str) -> Result<Self, FineractError> {
        let normalized = raw.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "clients" | "client" => Ok(Self::Clients),
            "staff" => Ok(Self::Staff),
            "loans" | "loan" => Ok(Self::Loans),
            "savings" | "savingsaccounts" | "savings_accounts" => Ok(Self::Savings),
            "client_identifiers" | "clientidentifiers" | "client-identifiers" => {
                Ok(Self::ClientIdentifiers)
            }
            "groups" | "group" => Ok(Self::Groups),
            "import" | "imports" => Ok(Self::Import),
            _ => Err(FineractError::DataValidation(
                PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.document.entityType.invalid",
                    format!("Unsupported document parent entity type: {raw}"),
                    DOCUMENT_API_PARAM_ENTITY_TYPE,
                    vec![ApiErrorArg::from(raw)],
                ),
            )),
        }
    }

    /// Returns `true` if this entity type supports profile images in `m_image` (`clients` or `staff`).
    pub const fn supports_images(self) -> bool {
        matches!(self, Self::Clients | Self::Staff)
    }
}

impl fmt::Display for DocumentParentEntityType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for DocumentParentEntityType {
    type Err = FineractError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

// =============================================================================
// DOMAIN & DTO MODELS (Document, DocumentData, ImageData, DocumentContent)
// =============================================================================

/// Persistent entity mapping to the `m_document` table.
///
/// Ported from `org.apache.fineract.infrastructure.documentmanagement.domain.Document`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: i64,
    pub parent_entity_type: String,
    pub parent_entity_id: i64,
    pub name: String,
    pub file_name: String,
    pub size: i64,
    #[serde(rename = "type", alias = "contentType", alias = "content_type")]
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub location: String,
    pub storage_type: StorageType,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            id: 0,
            parent_entity_type: String::new(),
            parent_entity_id: 0,
            name: String::new(),
            file_name: String::new(),
            size: 0,
            content_type: APPLICATION_OCTET_STREAM.to_string(),
            description: None,
            location: String::new(),
            storage_type: StorageType::FileSystem,
        }
    }
}

impl Document {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i64,
        parent_entity_type: impl Into<String>,
        parent_entity_id: i64,
        name: impl Into<String>,
        file_name: impl Into<String>,
        size: i64,
        content_type: impl Into<String>,
        description: Option<String>,
        location: impl Into<String>,
        storage_type: StorageType,
    ) -> Self {
        Self {
            id,
            parent_entity_type: parent_entity_type.into(),
            parent_entity_id,
            name: name.into(),
            file_name: file_name.into(),
            size,
            content_type: content_type.into(),
            description,
            location: location.into(),
            storage_type,
        }
    }

    /// Converts this domain [`Document`] into a wire-compatible [`DocumentData`] DTO
    /// (matching `DocumentMapper.map(Document)`).
    pub fn to_data(&self) -> DocumentData {
        DocumentData {
            id: self.id,
            parent_entity_type: self.parent_entity_type.clone(),
            parent_entity_id: self.parent_entity_id,
            name: self.name.clone(),
            file_name: self.file_name.clone(),
            size: self.size,
            content_type: self.content_type.clone(),
            description: self.description.clone(),
            location: self.location.clone(),
            storage_type: self.storage_type,
        }
    }
}

/// Wire-compatible DTO representing a document attachment (`m_document`).
///
/// Ported from `org.apache.fineract.infrastructure.documentmanagement.data.DocumentData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentData {
    pub id: i64,
    pub parent_entity_type: String,
    pub parent_entity_id: i64,
    pub name: String,
    pub file_name: String,
    pub size: i64,
    #[serde(rename = "type", alias = "contentType", alias = "content_type")]
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub location: String,
    pub storage_type: StorageType,
}

impl Default for DocumentData {
    fn default() -> Self {
        Document::default().to_data()
    }
}

impl DocumentData {
    /// Converts a [`Document`] into [`DocumentData`].
    pub fn from_document(doc: &Document) -> Self {
        doc.to_data()
    }
}

impl From<Document> for DocumentData {
    fn from(doc: Document) -> Self {
        DocumentData {
            id: doc.id,
            parent_entity_type: doc.parent_entity_type,
            parent_entity_id: doc.parent_entity_id,
            name: doc.name,
            file_name: doc.file_name,
            size: doc.size,
            content_type: doc.content_type,
            description: doc.description,
            location: doc.location,
            storage_type: doc.storage_type,
        }
    }
}

impl From<DocumentData> for Document {
    fn from(data: DocumentData) -> Self {
        Document {
            id: data.id,
            parent_entity_type: data.parent_entity_type,
            parent_entity_id: data.parent_entity_id,
            name: data.name,
            file_name: data.file_name,
            size: data.size,
            content_type: data.content_type,
            description: data.description,
            location: data.location,
            storage_type: data.storage_type,
        }
    }
}

/// Binary content payload associated with a document or image download.
///
/// Ported from `org.apache.fineract.infrastructure.documentmanagement.data.DocumentContent`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentContent {
    pub display_name: String,
    pub file_name: String,
    pub content_type: String,
    pub format: Option<String>,
    pub size: i64,
    #[serde(skip)]
    pub bytes: Vec<u8>,
}

/// Entity image metadata mapping to `m_image` and the owning entity (`m_client.image_id` or `m_staff.image_id`).
///
/// Ported from `org.apache.fineract.infrastructure.documentmanagement.domain.Image` and
/// `EntityImageIdAdapter`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageData {
    pub id: i64,
    pub location: String,
    pub storage_type: StorageType,
    pub entity_type: String,
    pub entity_id: i64,
}

/// Alias matching `org.apache.fineract.infrastructure.documentmanagement.domain.Image`.
pub type Image = ImageData;

impl Default for ImageData {
    fn default() -> Self {
        Self {
            id: 0,
            location: String::new(),
            storage_type: StorageType::FileSystem,
            entity_type: DEFAULT_IMAGE_ENTITY_TYPE.to_string(),
            entity_id: 0,
        }
    }
}

impl ImageData {
    pub fn new(
        id: i64,
        location: impl Into<String>,
        storage_type: StorageType,
        entity_type: impl Into<String>,
        entity_id: i64,
    ) -> Self {
        Self {
            id,
            location: location.into(),
            storage_type,
            entity_type: entity_type.into(),
            entity_id,
        }
    }

    /// Extracts the trailing file name from `self.location`.
    pub fn file_name(&self) -> &str {
        self.location
            .rsplit('/')
            .next()
            .unwrap_or(self.location.as_str())
    }
}

// =============================================================================
// CONTENT REPOSITORY VALIDATOR (Size Limits, Path Traversal, Base64 Data URI)
// =============================================================================

/// Decoded image payload extracted from a Base64 Data URI (`data:image/<ext>;base64,...`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecodedBase64Image {
    pub mime_type: String,
    pub extension: String,
    pub bytes: Vec<u8>,
}

/// Alias for [`DecodedBase64Image`].
pub type DecodedImageDataUri = DecodedBase64Image;

/// Validator enforcing Fineract content repository security policies:
/// - Maximum file size limits (`FileUploadValidator` / `ContentRepository`)
/// - Path traversal sanitization (`TraversalContentPolicy` / `DefaultContentPathSanitizer` rejecting `..`, `/`, `\`)
/// - Base64 Data URI decoding (`DataUrlDecoderContentProcessor` + `Base64DecoderContentProcessor` for
///   `data:image/png;base64,...`, `data:image/jpeg;base64,...`, `data:image/gif;base64,...`)
#[derive(Debug, Clone)]
pub struct ContentRepositoryValidator {
    max_file_size: i64,
    max_image_size: i64,
}

impl Default for ContentRepositoryValidator {
    fn default() -> Self {
        Self {
            max_file_size: Self::DEFAULT_MAX_FILE_SIZE,
            max_image_size: Self::DEFAULT_MAX_IMAGE_SIZE,
        }
    }
}

impl ContentRepositoryValidator {
    /// Default maximum document upload size: 5 MiB (`5 * 1024 * 1024` bytes).
    pub const DEFAULT_MAX_FILE_SIZE: i64 = 5 * 1024 * 1024;
    /// Default maximum entity image size: 5 MiB (`5 * 1024 * 1024` bytes).
    pub const DEFAULT_MAX_IMAGE_SIZE: i64 = 5 * 1024 * 1024;

    /// Allowed image MIME types matching `ImageCreateRequest.java` (`^(image\/gif|image\/jpeg|image\/png)$`).
    pub const ALLOWED_IMAGE_MIME_TYPES: &'static [&'static str] =
        &["image/png", "image/jpeg", "image/gif"];

    /// Creates a new [`ContentRepositoryValidator`] with the given maximum file size in bytes.
    pub fn new(max_file_size: i64) -> Self {
        Self {
            max_file_size,
            max_image_size: max_file_size,
        }
    }

    /// Creates a new [`ContentRepositoryValidator`] with distinct document and image size limits.
    pub fn with_limits(max_file_size: i64, max_image_size: i64) -> Self {
        Self {
            max_file_size,
            max_image_size,
        }
    }

    /// Returns the configured maximum document file size in bytes.
    pub const fn max_file_size(&self) -> i64 {
        self.max_file_size
    }

    /// Returns the configured maximum image file size in bytes.
    pub const fn max_image_size(&self) -> i64 {
        self.max_image_size
    }

    /// Validates an uploaded file size against `self.max_file_size` and ensures `size > 0`
    /// (ported from `FileUploadValidator.java` and `DocumentCreateRequest.java`).
    pub fn validate_file_size(&self, size: i64) -> Result<(), FineractError> {
        Self::check_file_size_with_limit(size, self.max_file_size)
    }

    /// Validates an uploaded image size against `self.max_image_size` and ensures `size > 0`.
    pub fn validate_image_size(&self, size: i64) -> Result<(), FineractError> {
        Self::check_file_size_with_limit(size, self.max_image_size)
    }

    /// Static helper validating `0 < size <= max_size`.
    pub fn check_file_size_with_limit(size: i64, max_size: i64) -> Result<(), FineractError> {
        let base_validator = DataValidatorBuilder::new().resource("fileUpload");
        base_validator
            .reset()
            .parameter("Content-Length")
            .value(size)
            .not_null()
            .integer_greater_than_number(0)
            .throw_validation_errors()?;

        if size > max_size {
            return Err(FineractError::DataValidation(
                PlatformApiDataValidationException::new(vec![ApiParameterError::parameter_error(
                    "error.msg.document.file.size.exceeded",
                    format!(
                        "File size {size} bytes exceeds maximum allowed size of {max_size} bytes"
                    ),
                    DOCUMENT_API_PARAM_FILE,
                    vec![ApiErrorArg::from(size), ApiErrorArg::from(max_size)],
                )]),
            ));
        }

        Ok(())
    }

    /// Sanitizes and validates a single file name, rejecting empty names, null bytes,
    /// directory separators (`/`, `\`), and parent directory traversal (`..`).
    pub fn sanitize_file_name(file_name: &str) -> Result<String, FineractError> {
        let trimmed = file_name.trim();
        if trimmed.is_empty() {
            return Err(Self::content_policy_error(
                "File name must not be empty".to_string(),
            ));
        }

        if trimmed.len() > 250 {
            return Err(Self::content_policy_error(format!(
                "File name exceeds maximum length of 250 characters: {trimmed}"
            )));
        }

        if trimmed.contains("..")
            || trimmed.contains('/')
            || trimmed.contains('\\')
            || trimmed.contains('\0')
            || trimmed == "."
        {
            return Err(Self::content_policy_error(format!(
                "Trying to overwrite a sibling file: {file_name}"
            )));
        }

        Ok(trimmed.to_string())
    }

    /// Instance method wrapper for [`Self::sanitize_file_name`].
    pub fn validate_file_name(&self, file_name: &str) -> Result<String, FineractError> {
        Self::sanitize_file_name(file_name)
    }

    /// Validates a storage path against path traversal (`TraversalContentPolicy.java`),
    /// rejecting `..`, `\`, leading `/`, empty segments (`//`), and null bytes.
    pub fn validate_storage_path(path: &str) -> Result<String, FineractError> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return Err(Self::content_policy_error(
                "Storage path must not be empty".to_string(),
            ));
        }

        // Matches TraversalContentPolicy.java pattern `.*((\.)?\./+)+(/+)?.*` plus strict checks
        let overwrite_siblings_re =
            Regex::new(r"^.*((\.)?\./+)+(/+)?.*$").expect("valid traversal regex");
        if overwrite_siblings_re.is_match(trimmed)
            || trimmed.contains("..")
            || trimmed.contains('\\')
            || trimmed.starts_with('/')
            || trimmed.ends_with('/')
            || trimmed.contains("//")
            || trimmed.contains('\0')
        {
            return Err(Self::content_policy_error(format!(
                "Trying to overwrite a sibling file: {path}"
            )));
        }

        let mut normalized_segments = Vec::new();
        for component in Path::new(trimmed).components() {
            match component {
                Component::Normal(os_str) => {
                    let seg = os_str.to_str().ok_or_else(|| {
                        Self::content_policy_error(format!("Invalid UTF-8 in path: {path}"))
                    })?;
                    if seg.is_empty() || seg == "." || seg == ".." {
                        return Err(Self::content_policy_error(format!(
                            "Trying to overwrite a sibling file: {path}"
                        )));
                    }
                    normalized_segments.push(seg);
                }
                _ => {
                    return Err(Self::content_policy_error(format!(
                        "Trying to overwrite a sibling file: {path}"
                    )));
                }
            }
        }

        if normalized_segments.is_empty() {
            return Err(Self::content_policy_error(
                "Storage path must contain at least one segment".to_string(),
            ));
        }

        Ok(normalized_segments.join("/"))
    }

    /// Instance method wrapper for [`Self::validate_storage_path`].
    pub fn validate_path(&self, path: &str) -> Result<String, FineractError> {
        Self::validate_storage_path(path)
    }

    /// Builds a validated relative content store path: `{prefix}/{entity_type}/{entity_id}/{file_name}`
    /// (matching `DocumentWritePlatformServiceImpl.getPath` and `ImageWritePlatformServiceImpl.getPath`).
    pub fn build_entity_storage_path(
        prefix: &str,
        entity_type: &str,
        entity_id: i64,
        file_name: &str,
    ) -> Result<String, FineractError> {
        if entity_id <= 0 {
            return Err(FineractError::DataValidation(
                PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.document.entityId.invalid",
                    "Entity ID must be greater than zero",
                    DOCUMENT_API_PARAM_ENTITY_ID,
                    vec![ApiErrorArg::from(entity_id)],
                ),
            ));
        }
        let clean_prefix = Self::sanitize_file_name(prefix)?;
        let clean_entity = Self::sanitize_file_name(entity_type)?;
        let clean_file_name = Self::sanitize_file_name(file_name)?;
        let path = format!("{clean_prefix}/{clean_entity}/{entity_id}/{clean_file_name}");
        Self::validate_storage_path(&path)
    }

    /// Decodes and validates a Base64 image Data URI (`data:image/png;base64,...`,
    /// `data:image/jpeg;base64,...`, `data:image/gif;base64,...`) enforcing `self.max_image_size`.
    ///
    /// Ported from `DataUrlDecoderContentProcessor`, `Base64DecoderContentProcessor`, and
    /// `ImageCreateRequest`.
    pub fn decode_base64_data_uri(
        &self,
        data_uri: &str,
    ) -> Result<DecodedBase64Image, FineractError> {
        Self::decode_base64_data_uri_with_limit(data_uri, self.max_image_size)
    }

    /// Static helper decoding a Base64 image Data URI with an explicit maximum byte size limit.
    pub fn decode_base64_data_uri_with_limit(
        data_uri: &str,
        max_size: i64,
    ) -> Result<DecodedBase64Image, FineractError> {
        let trimmed = data_uri.trim();
        let Some( comma_idx ) = trimmed.find(',') else {
            return Err(Self::content_policy_error(
                "Invalid Data URI: missing ',' separator".to_string(),
            ));
        };

        let header = &trimmed[..comma_idx];
        let payload = &trimmed[comma_idx + 1..];

        if !header.starts_with("data:") {
            return Err(Self::content_policy_error(
                "Invalid Data URI: must start with 'data:'".to_string(),
            ));
        }

        let meta = &header["data:".len()..];
        let Some(raw_mime) = meta.strip_suffix(";base64") else {
            return Err(Self::content_policy_error(
                "Invalid Data URI: missing ';base64' encoding specifier".to_string(),
            ));
        };

        let mime_lower = raw_mime.trim().to_ascii_lowercase();
        let (canonical_mime, extension) = match mime_lower.as_str() {
            "image/png" => ("image/png", "png"),
            "image/jpeg" | "image/jpg" => ("image/jpeg", "jpg"),
            "image/gif" => ("image/gif", "gif"),
            other => {
                return Err(Self::content_policy_error(format!(
                    "Detected mime type {other} not allowed! Allowed image types: image/png, image/jpeg, image/gif"
                )));
            }
        };

        let decoded_bytes = decode_base64(payload)?;
        if decoded_bytes.is_empty() {
            return Err(Self::content_policy_error(
                "Decoded Base64 image payload must not be empty".to_string(),
            ));
        }

        Self::check_file_size_with_limit(decoded_bytes.len() as i64, max_size)?;

        Ok(DecodedBase64Image {
            mime_type: canonical_mime.to_string(),
            extension: extension.to_string(),
            bytes: decoded_bytes,
        })
    }

    /// Encodes binary image bytes into a Data URI string (`data:{mime_type};base64,{base64}`)
    /// matching `Base64EncoderContentProcessor.then(DataUrlEncoderContentProcessor)` in `ImagesApiResource`.
    pub fn encode_base64_data_uri(mime_type: &str, bytes: &[u8]) -> String {
        let encoded = encode_base64(bytes);
        format!("data:{mime_type};base64,{encoded}")
    }

    /// Infers a MIME type from a file name extension (ported from `TikaContentDetector` fallback).
    pub fn detect_mime_type_from_file_name(file_name: &str) -> &'static str {
        let ext = file_name
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "pdf" => "application/pdf",
            "txt" => "text/plain",
            "csv" => "text/csv",
            "json" => "application/json",
            "doc" => "application/msword",
            "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "xls" => "application/vnd.ms-excel",
            "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            _ => APPLICATION_OCTET_STREAM,
        }
    }

    /// Constructs a `PlatformApiDataValidationException` matching `DocumentWritePlatformServiceImpl.upload`:
    /// `new PlatformApiDataValidationException("error.msg.content.policy", e.getDefaultUserMessage(), "file", e)`.
    pub fn content_policy_error(message: impl Into<String>) -> FineractError {
        let msg = message.into();
        FineractError::DataValidation(PlatformApiDataValidationException::new(vec![
            ApiParameterError::parameter_error(
                "error.msg.content.policy",
                msg,
                DOCUMENT_API_PARAM_FILE,
                Vec::new(),
            ),
        ]))
    }

    /// Constructs a `DocumentNotFoundException` (`FineractError::ResourceNotFound`) matching
    /// `DocumentNotFoundException(entityType, entityId, id)`.
    pub fn document_not_found(entity_type: &str, entity_id: i64, id: i64) -> FineractError {
        FineractError::resource_not_found(
            "error.msg.document.id.invalid",
            format!(
                "Document with identifier {id} does not exist for the {entity_type} with Identifier {entity_id}"
            ),
            id.to_string(),
        )
    }
}

// =============================================================================
// RFC 4648 BASE64 ENCODER & DECODER
// =============================================================================

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encodes arbitrary bytes into standard RFC 4648 Base64 with `=` padding.
pub fn encode_base64(input: &[u8]) -> String {
    if input.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(BASE64_ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        out.push(BASE64_ALPHABET[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(BASE64_ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(BASE64_ALPHABET[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Decodes standard RFC 4648 Base64 (ignoring ASCII whitespace `\r`, `\n`, `' '`, `\t`) into bytes.
pub fn decode_base64(input: &str) -> Result<Vec<u8>, FineractError> {
    let filtered: Vec<u8> = input
        .bytes()
        .filter(|b| !matches!(b, b' ' | b'\n' | b'\r' | b'\t'))
        .collect();

    if filtered.is_empty() {
        return Ok(Vec::new());
    }

    if filtered.len() % 4 != 0 {
        return Err(ContentRepositoryValidator::content_policy_error(
            "Invalid Base64 length: must be a multiple of 4".to_string(),
        ));
    }

    let mut out = Vec::with_capacity((filtered.len() / 4) * 3);
    for (chunk_idx, chunk) in filtered.chunks_exact(4).enumerate() {
        let is_last = chunk_idx + 1 == filtered.len() / 4;
        let mut vals = [0u8; 4];
        let mut pad_count = 0usize;

        for (i, &ch) in chunk.iter().enumerate() {
            if ch == b'=' {
                if !is_last || (i < 2) {
                    return Err(ContentRepositoryValidator::content_policy_error(
                        "Invalid Base64 padding position".to_string(),
                    ));
                }
                pad_count += 1;
                vals[i] = 0;
            } else {
                if pad_count > 0 {
                    return Err(ContentRepositoryValidator::content_policy_error(
                        "Invalid Base64 character after '=' padding".to_string(),
                    ));
                }
                vals[i] = decode_base64_sextet(ch).ok_or_else(|| {
                    ContentRepositoryValidator::content_policy_error(format!(
                        "Invalid Base64 character: {:?}",
                        ch as char
                    ))
                })?;
            }
        }

        let triple = ((vals[0] as u32) << 18)
            | ((vals[1] as u32) << 12)
            | ((vals[2] as u32) << 6)
            | (vals[3] as u32);

        out.push(((triple >> 16) & 0xFF) as u8);
        if pad_count < 2 {
            out.push(((triple >> 8) & 0xFF) as u8);
        }
        if pad_count == 0 {
            out.push((triple & 0xFF) as u8);
        }
    }

    Ok(out)
}

const fn decode_base64_sextet(ch: u8) -> Option<u8> {
    match ch {
        b'A'..=b'Z' => Some(ch - b'A'),
        b'a'..=b'z' => Some(ch - b'a' + 26),
        b'0'..=b'9' => Some(ch - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

// =============================================================================
// CONTENT STORE TRAIT & IMPLEMENTATIONS
// =============================================================================

/// Asynchronous binary content storage abstraction (`FileSystem` / `S3` / `InMemory`).
///
/// Ported from `org.apache.fineract.infrastructure.contentstore.service.ContentStoreService`.
#[async_trait]
pub trait ContentStore: Send + Sync {
    /// Returns the [`StorageType`] discriminant for this content store.
    fn storage_type(&self) -> StorageType {
        StorageType::FileSystem
    }

    /// Saves binary file content at `location` after validating against path traversal.
    /// Returns the normalized storage location.
    async fn save_file(&self, location: &str, content: &[u8]) -> Result<String, FineractError>;

    /// Reads binary file content from `location`.
    async fn read_file(&self, location: &str) -> Result<Vec<u8>, FineractError>;

    /// Deletes the file at `location`.
    async fn delete_file(&self, location: &str) -> Result<(), FineractError>;
}

/// Concurrent in-memory [`ContentStore`] implementation backed by [`DashMap`].
#[derive(Debug)]
pub struct InMemoryContentStore {
    files: DashMap<String, Vec<u8>>,
    storage_type: StorageType,
    validator: ContentRepositoryValidator,
}

impl Default for InMemoryContentStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryContentStore {
    /// Creates an empty [`InMemoryContentStore`] with default [`ContentRepositoryValidator`] limits.
    pub fn new() -> Self {
        Self {
            files: DashMap::new(),
            storage_type: StorageType::FileSystem,
            validator: ContentRepositoryValidator::default(),
        }
    }

    /// Creates an [`InMemoryContentStore`] with a custom [`ContentRepositoryValidator`].
    pub fn with_validator(validator: ContentRepositoryValidator) -> Self {
        Self {
            files: DashMap::new(),
            storage_type: StorageType::FileSystem,
            validator,
        }
    }

    /// Sets the reported [`StorageType`] (e.g., `StorageType::S3` for S3 simulation tests).
    pub fn with_storage_type(mut self, storage_type: StorageType) -> Self {
        self.storage_type = storage_type;
        self
    }

    /// Returns the number of stored files.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Returns `true` if no files are currently stored.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Returns `true` if a file exists at `location`.
    pub fn contains(&self, location: &str) -> bool {
        self.files.contains_key(location)
    }
}

#[async_trait]
impl ContentStore for InMemoryContentStore {
    fn storage_type(&self) -> StorageType {
        self.storage_type
    }

    async fn save_file(&self, location: &str, content: &[u8]) -> Result<String, FineractError> {
        let clean_path = self.validator.validate_path(location)?;
        self.validator.validate_file_size(content.len() as i64)?;
        self.files.insert(clean_path.clone(), content.to_vec());
        Ok(clean_path)
    }

    async fn read_file(&self, location: &str) -> Result<Vec<u8>, FineractError> {
        let clean_path = self.validator.validate_path(location)?;
        self.files
            .get(&clean_path)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| {
                FineractError::resource_not_found(
                    "error.msg.document.file.not.found",
                    format!("Content file at location '{clean_path}' does not exist"),
                    clean_path,
                )
            })
    }

    async fn delete_file(&self, location: &str) -> Result<(), FineractError> {
        let clean_path = self.validator.validate_path(location)?;
        self.files.remove(&clean_path);
        Ok(())
    }
}

/// Local filesystem [`ContentStore`] implementation ported from
/// `org.apache.fineract.infrastructure.contentstore.service.FileContentStoreService`.
#[derive(Debug, Clone)]
pub struct LocalFileSystemContentStore {
    root_dir: PathBuf,
    validator: ContentRepositoryValidator,
}

impl LocalFileSystemContentStore {
    /// Creates a new [`LocalFileSystemContentStore`] rooted at `root_dir`.
    pub fn new(root_dir: impl Into<PathBuf>) -> Self {
        Self {
            root_dir: root_dir.into(),
            validator: ContentRepositoryValidator::default(),
        }
    }

    /// Creates a new [`LocalFileSystemContentStore`] with a custom [`ContentRepositoryValidator`].
    pub fn with_validator(
        root_dir: impl Into<PathBuf>,
        validator: ContentRepositoryValidator,
    ) -> Self {
        Self {
            root_dir: root_dir.into(),
            validator,
        }
    }

    /// Returns the root directory of the content store.
    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    /// Resolves a relative storage location into a safe path confined within `self.root_dir`.
    pub fn resolve_safe_path(&self, location: &str) -> Result<(String, PathBuf), FineractError> {
        let clean_rel = self.validator.validate_path(location)?;
        let mut full_path = self.root_dir.clone();
        for segment in clean_rel.split('/') {
            full_path.push(segment);
        }
        if !full_path.starts_with(&self.root_dir) {
            return Err(ContentRepositoryValidator::content_policy_error(format!(
                "Trying to overwrite a sibling file: {location}"
            )));
        }
        Ok((clean_rel, full_path))
    }
}

#[async_trait]
impl ContentStore for LocalFileSystemContentStore {
    fn storage_type(&self) -> StorageType {
        StorageType::FileSystem
    }

    async fn save_file(&self, location: &str, content: &[u8]) -> Result<String, FineractError> {
        let (clean_rel, full_path) = self.resolve_safe_path(location)?;
        self.validator.validate_file_size(content.len() as i64)?;

        if let Some(parent) = full_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| FineractError::DatabaseError(format!("ContentStore mkdir failed: {e}")))?;
        }

        tokio::fs::write(&full_path, content)
            .await
            .map_err(|e| FineractError::DatabaseError(format!("ContentStore write failed: {e}")))?;

        Ok(clean_rel)
    }

    async fn read_file(&self, location: &str) -> Result<Vec<u8>, FineractError> {
        let (clean_rel, full_path) = self.resolve_safe_path(location)?;
        match tokio::fs::read(&full_path).await {
            Ok(bytes) => Ok(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(FineractError::resource_not_found(
                    "error.msg.document.file.not.found",
                    format!("Content file at location '{clean_rel}' does not exist"),
                    clean_rel,
                ))
            }
            Err(e) => Err(FineractError::DatabaseError(format!(
                "ContentStore read failed: {e}"
            ))),
        }
    }

    async fn delete_file(&self, location: &str) -> Result<(), FineractError> {
        let (_, full_path) = self.resolve_safe_path(location)?;
        match tokio::fs::remove_file(&full_path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(FineractError::DatabaseError(format!(
                "ContentStore delete failed: {e}"
            ))),
        }
    }
}

// =============================================================================
// DOCUMENT & IMAGE REPOSITORY (InMemory + DualDb SQL Queries)
// =============================================================================

/// Async persistence contract for `m_document` and `m_image` records.
#[async_trait]
pub trait DocumentMetadataRepository: Send + Sync {
    async fn list_documents(
        &self,
        parent_entity_type: &str,
        parent_entity_id: i64,
    ) -> Result<Vec<Document>, FineractError>;

    async fn find_document_by_id(&self, id: i64) -> Result<Option<Document>, FineractError>;

    async fn find_document(
        &self,
        parent_entity_type: &str,
        parent_entity_id: i64,
        id: i64,
    ) -> Result<Option<Document>, FineractError>;

    async fn insert_document(&self, document: Document) -> Result<Document, FineractError>;

    async fn update_document(&self, document: Document) -> Result<Document, FineractError>;

    async fn delete_document(
        &self,
        parent_entity_type: &str,
        parent_entity_id: i64,
        id: i64,
    ) -> Result<(), FineractError>;

    async fn find_image_by_entity(
        &self,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<Option<ImageData>, FineractError>;

    async fn upsert_image(&self, image: ImageData) -> Result<ImageData, FineractError>;

    async fn delete_image(
        &self,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<Option<ImageData>, FineractError>;
}

/// Concurrent in-memory [`DocumentMetadataRepository`] implementation backed by [`DashMap`].
#[derive(Debug)]
pub struct InMemoryDocumentRepository {
    documents: DashMap<i64, Document>,
    images_by_entity: DashMap<(String, i64), ImageData>,
    next_doc_id: AtomicI64,
    next_image_id: AtomicI64,
}

impl Default for InMemoryDocumentRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryDocumentRepository {
    pub fn new() -> Self {
        Self {
            documents: DashMap::new(),
            images_by_entity: DashMap::new(),
            next_doc_id: AtomicI64::new(1),
            next_image_id: AtomicI64::new(1),
        }
    }

    pub fn document_count(&self) -> usize {
        self.documents.len()
    }

    pub fn image_count(&self) -> usize {
        self.images_by_entity.len()
    }
}

#[async_trait]
impl DocumentMetadataRepository for InMemoryDocumentRepository {
    async fn list_documents(
        &self,
        parent_entity_type: &str,
        parent_entity_id: i64,
    ) -> Result<Vec<Document>, FineractError> {
        let mut docs: Vec<Document> = self
            .documents
            .iter()
            .filter(|entry| {
                entry
                    .value()
                    .parent_entity_type
                    .eq_ignore_ascii_case(parent_entity_type)
                    && entry.value().parent_entity_id == parent_entity_id
            })
            .map(|entry| entry.value().clone())
            .collect();
        docs.sort_by_key(|d| d.id);
        Ok(docs)
    }

    async fn find_document_by_id(&self, id: i64) -> Result<Option<Document>, FineractError> {
        Ok(self.documents.get(&id).map(|e| e.value().clone()))
    }

    async fn find_document(
        &self,
        parent_entity_type: &str,
        parent_entity_id: i64,
        id: i64,
    ) -> Result<Option<Document>, FineractError> {
        Ok(self.documents.get(&id).and_then(|e| {
            let doc = e.value();
            if doc
                .parent_entity_type
                .eq_ignore_ascii_case(parent_entity_type)
                && doc.parent_entity_id == parent_entity_id
            {
                Some(doc.clone())
            } else {
                None
            }
        }))
    }

    async fn insert_document(&self, mut document: Document) -> Result<Document, FineractError> {
        let id = if document.id > 0 {
            document.id
        } else {
            self.next_doc_id.fetch_add(1, Ordering::SeqCst)
        };
        document.id = id;
        self.documents.insert(id, document.clone());
        Ok(document)
    }

    async fn update_document(&self, document: Document) -> Result<Document, FineractError> {
        if !self.documents.contains_key(&document.id) {
            return Err(ContentRepositoryValidator::document_not_found(
                &document.parent_entity_type,
                document.parent_entity_id,
                document.id,
            ));
        }
        self.documents.insert(document.id, document.clone());
        Ok(document)
    }

    async fn delete_document(
        &self,
        parent_entity_type: &str,
        parent_entity_id: i64,
        id: i64,
    ) -> Result<(), FineractError> {
        let existing = self
            .find_document(parent_entity_type, parent_entity_id, id)
            .await?;
        if existing.is_none() {
            return Err(ContentRepositoryValidator::document_not_found(
                parent_entity_type,
                parent_entity_id,
                id,
            ));
        }
        self.documents.remove(&id);
        Ok(())
    }

    async fn find_image_by_entity(
        &self,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<Option<ImageData>, FineractError> {
        let key = (entity_type.to_ascii_lowercase(), entity_id);
        Ok(self.images_by_entity.get(&key).map(|e| e.value().clone()))
    }

    async fn upsert_image(&self, mut image: ImageData) -> Result<ImageData, FineractError> {
        let key = (image.entity_type.to_ascii_lowercase(), image.entity_id);
        let id = if let Some(existing) = self.images_by_entity.get(&key) {
            existing.value().id
        } else if image.id > 0 {
            image.id
        } else {
            self.next_image_id.fetch_add(1, Ordering::SeqCst)
        };
        image.id = id;
        image.entity_type = key.0.clone();
        self.images_by_entity.insert(key, image.clone());
        Ok(image)
    }

    async fn delete_image(
        &self,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<Option<ImageData>, FineractError> {
        let key = (entity_type.to_ascii_lowercase(), entity_id);
        Ok(self.images_by_entity.remove(&key).map(|(_, img)| img))
    }
}

/// Dual-Database (`PostgreSQL` + `MySQL/MariaDB`) SQLx repository for `m_document` and `m_image`.
///
/// Ported from `DocumentRepository`, `ImageRepository`, `ClientImageIdAdapter`, and `StaffImageIdAdapter`.
#[derive(Debug, Clone)]
pub struct DualDbDocumentRepository {
    pool: AnyPool,
    dialect: SqlDialect,
}

impl DualDbDocumentRepository {
    pub const DOCUMENT_COLUMNS: &'static str = "id, parent_entity_type, parent_entity_id, name, \
        file_name, size, type, description, location, storage_type_enum";

    pub const IMAGE_COLUMNS: &'static str = "id, location, storage_type_enum";

    pub fn new(pool: AnyPool, dialect: SqlDialect) -> Self {
        Self { pool, dialect }
    }

    pub const fn dialect(&self) -> SqlDialect {
        self.dialect
    }

    pub const fn pool(&self) -> &AnyPool {
        &self.pool
    }

    /// Generates dialect-specific SQL for listing documents attached to `(parent_entity_type, parent_entity_id)`.
    pub fn select_documents_by_entity_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM m_document WHERE parent_entity_type = ? AND parent_entity_id = ? ORDER BY id ASC",
            Self::DOCUMENT_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    /// Generates dialect-specific SQL for selecting a single document by `id`.
    pub fn select_document_by_id_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM m_document WHERE id = ?",
            Self::DOCUMENT_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    /// Generates dialect-specific SQL for selecting a document by `(id, parent_entity_type, parent_entity_id)`.
    pub fn select_document_by_entity_and_id_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM m_document WHERE id = ? AND parent_entity_type = ? AND parent_entity_id = ?",
            Self::DOCUMENT_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    /// Generates dialect-specific SQL for inserting a row into `m_document`.
    pub fn insert_document_sql(dialect: SqlDialect) -> String {
        let raw = "INSERT INTO m_document (\
            id, parent_entity_type, parent_entity_id, name, file_name, size, type, \
            description, location, storage_type_enum\
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    /// Generates dialect-specific SQL for updating a row in `m_document`.
    pub fn update_document_sql(dialect: SqlDialect) -> String {
        let raw = "UPDATE m_document SET \
            name = ?, file_name = ?, size = ?, type = ?, description = ?, location = ?, \
            storage_type_enum = ? \
            WHERE id = ? AND parent_entity_type = ? AND parent_entity_id = ?";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    /// Generates dialect-specific SQL for deleting a row from `m_document`.
    pub fn delete_document_sql(dialect: SqlDialect) -> String {
        let raw =
            "DELETE FROM m_document WHERE id = ? AND parent_entity_type = ? AND parent_entity_id = ?";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    /// Generates dialect-specific SQL for selecting an `m_image` row by primary key `id`.
    pub fn select_image_by_id_sql(dialect: SqlDialect) -> String {
        let raw = format!("SELECT {} FROM m_image WHERE id = ?", Self::IMAGE_COLUMNS);
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    /// Generates dialect-specific SQL for selecting an `m_image` joined through `m_client.image_id` or `m_staff.image_id`.
    pub fn select_entity_image_sql(entity_type: &str, dialect: SqlDialect) -> Result<String, FineractError> {
        let table = Self::entity_image_owner_table(entity_type)?;
        let raw = format!(
            "SELECT i.id, i.location, i.storage_type_enum FROM m_image i INNER JOIN {table} e ON e.image_id = i.id WHERE e.id = ?"
        );
        Ok(DialectSql::rewrite_placeholders(&raw, dialect))
    }

    /// Generates dialect-specific SQL for inserting a row into `m_image`.
    pub fn insert_image_sql(dialect: SqlDialect) -> String {
        let raw = "INSERT INTO m_image (id, location, storage_type_enum) VALUES (?, ?, ?)";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    /// Generates dialect-specific SQL for updating a row in `m_image`.
    pub fn update_image_sql(dialect: SqlDialect) -> String {
        let raw = "UPDATE m_image SET location = ?, storage_type_enum = ? WHERE id = ?";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    /// Generates dialect-specific SQL for deleting a row from `m_image`.
    pub fn delete_image_sql(dialect: SqlDialect) -> String {
        let raw = "DELETE FROM m_image WHERE id = ?";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    /// Generates dialect-specific SQL for linking or unlinking `image_id` on `m_client` or `m_staff`.
    pub fn update_entity_image_id_sql(
        entity_type: &str,
        dialect: SqlDialect,
    ) -> Result<String, FineractError> {
        let table = Self::entity_image_owner_table(entity_type)?;
        let raw = format!("UPDATE {table} SET image_id = ? WHERE id = ?");
        Ok(DialectSql::rewrite_placeholders(&raw, dialect))
    }

    fn entity_image_owner_table(entity_type: &str) -> Result<&'static str, FineractError> {
        match entity_type.trim().to_ascii_lowercase().as_str() {
            "clients" | "client" => Ok("m_client"),
            "staff" => Ok("m_staff"),
            other => Err(FineractError::DataValidation(
                PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.image.entityType.invalid",
                    format!("Entity type '{other}' does not support profile images; expected 'clients' or 'staff'"),
                    DOCUMENT_API_PARAM_ENTITY_TYPE,
                    vec![ApiErrorArg::from(other)],
                ),
            )),
        }
    }

    fn map_document_row(row: &sqlx::any::AnyRow) -> Result<Document, FineractError> {
        let id: i64 = row
            .try_get::<i64, _>("id")
            .or_else(|_| row.try_get::<i32, _>("id").map(i64::from))
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let parent_entity_id: i64 = row
            .try_get::<i64, _>("parent_entity_id")
            .or_else(|_| row.try_get::<i32, _>("parent_entity_id").map(i64::from))
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let size: i64 = row
            .try_get::<Option<i64>, _>("size")
            .or_else(|_| {
                row.try_get::<Option<i32>, _>("size")
                    .map(|opt| opt.map(i64::from))
            })
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?
            .unwrap_or(0);
        let storage_discriminant: Option<i16> = row
            .try_get::<Option<i16>, _>("storage_type_enum")
            .or_else(|_| {
                row.try_get::<Option<i32>, _>("storage_type_enum")
                    .map(|opt| opt.map(|v| v as i16))
            })
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let storage_type = storage_discriminant
            .and_then(StorageType::from_i16)
            .unwrap_or(StorageType::FileSystem);

        Ok(Document {
            id,
            parent_entity_type: row
                .try_get("parent_entity_type")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            parent_entity_id,
            name: row
                .try_get("name")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            file_name: row
                .try_get("file_name")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            size,
            content_type: row
                .try_get::<Option<String>, _>("type")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?
                .unwrap_or_else(|| APPLICATION_OCTET_STREAM.to_string()),
            description: row
                .try_get("description")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            location: row
                .try_get("location")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            storage_type,
        })
    }

    fn map_image_row(
        row: &sqlx::any::AnyRow,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<ImageData, FineractError> {
        let id: i64 = row
            .try_get::<i64, _>("id")
            .or_else(|_| row.try_get::<i32, _>("id").map(i64::from))
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let location: String = row
            .try_get::<Option<String>, _>("location")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?
            .unwrap_or_default();
        let storage_discriminant: Option<i16> = row
            .try_get::<Option<i16>, _>("storage_type_enum")
            .or_else(|_| {
                row.try_get::<Option<i32>, _>("storage_type_enum")
                    .map(|opt| opt.map(|v| v as i16))
            })
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let storage_type = storage_discriminant
            .and_then(StorageType::from_i16)
            .unwrap_or(StorageType::FileSystem);

        Ok(ImageData {
            id,
            location,
            storage_type,
            entity_type: entity_type.to_ascii_lowercase(),
            entity_id,
        })
    }
}

#[async_trait]
impl DocumentMetadataRepository for DualDbDocumentRepository {
    async fn list_documents(
        &self,
        parent_entity_type: &str,
        parent_entity_id: i64,
    ) -> Result<Vec<Document>, FineractError> {
        let sql = Self::select_documents_by_entity_sql(self.dialect);
        let rows = sqlx::query(&sql)
            .bind(parent_entity_type)
            .bind(parent_entity_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        rows.iter().map(Self::map_document_row).collect()
    }

    async fn find_document_by_id(&self, id: i64) -> Result<Option<Document>, FineractError> {
        let sql = Self::select_document_by_id_sql(self.dialect);
        let maybe_row = sqlx::query(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        match maybe_row {
            Some(ref row) => Ok(Some(Self::map_document_row(row)?)),
            None => Ok(None),
        }
    }

    async fn find_document(
        &self,
        parent_entity_type: &str,
        parent_entity_id: i64,
        id: i64,
    ) -> Result<Option<Document>, FineractError> {
        let sql = Self::select_document_by_entity_and_id_sql(self.dialect);
        let maybe_row = sqlx::query(&sql)
            .bind(id)
            .bind(parent_entity_type)
            .bind(parent_entity_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        match maybe_row {
            Some(ref row) => Ok(Some(Self::map_document_row(row)?)),
            None => Ok(None),
        }
    }

    async fn insert_document(&self, document: Document) -> Result<Document, FineractError> {
        let sql = Self::insert_document_sql(self.dialect);
        sqlx::query(&sql)
            .bind(document.id)
            .bind(&document.parent_entity_type)
            .bind(document.parent_entity_id)
            .bind(&document.name)
            .bind(&document.file_name)
            .bind(document.size)
            .bind(&document.content_type)
            .bind(&document.description)
            .bind(&document.location)
            .bind(document.storage_type.as_i16())
            .execute(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        Ok(document)
    }

    async fn update_document(&self, document: Document) -> Result<Document, FineractError> {
        let sql = Self::update_document_sql(self.dialect);
        sqlx::query(&sql)
            .bind(&document.name)
            .bind(&document.file_name)
            .bind(document.size)
            .bind(&document.content_type)
            .bind(&document.description)
            .bind(&document.location)
            .bind(document.storage_type.as_i16())
            .bind(document.id)
            .bind(&document.parent_entity_type)
            .bind(document.parent_entity_id)
            .execute(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        Ok(document)
    }

    async fn delete_document(
        &self,
        parent_entity_type: &str,
        parent_entity_id: i64,
        id: i64,
    ) -> Result<(), FineractError> {
        let sql = Self::delete_document_sql(self.dialect);
        sqlx::query(&sql)
            .bind(id)
            .bind(parent_entity_type)
            .bind(parent_entity_id)
            .execute(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn find_image_by_entity(
        &self,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<Option<ImageData>, FineractError> {
        let sql = Self::select_entity_image_sql(entity_type, self.dialect)?;
        let maybe_row = sqlx::query(&sql)
            .bind(entity_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        match maybe_row {
            Some(ref row) => Ok(Some(Self::map_image_row(row, entity_type, entity_id)?)),
            None => Ok(None),
        }
    }

    async fn upsert_image(&self, image: ImageData) -> Result<ImageData, FineractError> {
        if let Some(existing) = self
            .find_image_by_entity(&image.entity_type, image.entity_id)
            .await?
        {
            let update_sql = Self::update_image_sql(self.dialect);
            sqlx::query(&update_sql)
                .bind(&image.location)
                .bind(image.storage_type.as_i16())
                .bind(existing.id)
                .execute(&self.pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

            Ok(ImageData {
                id: existing.id,
                ..image
            })
        } else {
            let insert_sql = Self::insert_image_sql(self.dialect);
            sqlx::query(&insert_sql)
                .bind(image.id)
                .bind(&image.location)
                .bind(image.storage_type.as_i16())
                .execute(&self.pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

            let link_sql = Self::update_entity_image_id_sql(&image.entity_type, self.dialect)?;
            sqlx::query(&link_sql)
                .bind(Some(image.id))
                .bind(image.entity_id)
                .execute(&self.pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

            Ok(image)
        }
    }

    async fn delete_image(
        &self,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<Option<ImageData>, FineractError> {
        let Some(existing) = self.find_image_by_entity(entity_type, entity_id).await? else {
            return Ok(None);
        };

        let unlink_sql = Self::update_entity_image_id_sql(entity_type, self.dialect)?;
        sqlx::query(&unlink_sql)
            .bind(None::<i64>)
            .bind(entity_id)
            .execute(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        let del_sql = Self::delete_image_sql(self.dialect);
        sqlx::query(&del_sql)
            .bind(existing.id)
            .execute(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        Ok(Some(existing))
    }
}

// =============================================================================
// DOCUMENT & IMAGE SERVICE + CQRS COMMAND HANDLERS
// =============================================================================

/// Request payload for creating a new document attachment (`DocumentCreateRequest.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentCreateRequest {
    pub entity_type: String,
    pub entity_id: i64,
    pub name: Option<String>,
    pub file_name: String,
    pub content_type: Option<String>,
    pub description: Option<String>,
    pub content: Vec<u8>,
}

/// Request payload for updating an existing document (`DocumentUpdateRequest.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentUpdateRequest {
    pub id: i64,
    pub entity_type: String,
    pub entity_id: i64,
    pub name: Option<String>,
    pub file_name: Option<String>,
    pub content_type: Option<String>,
    pub description: Option<String>,
    pub content: Option<Vec<u8>>,
}

/// Service orchestrating `DocumentMetadataRepository`, `ContentStore`, and `ContentRepositoryValidator`.
///
/// Ported from `DocumentReadPlatformServiceImpl`, `DocumentWritePlatformServiceImpl`,
/// `ImageReadPlatformServiceImpl`, and `ImageWritePlatformServiceImpl`.
pub struct DocumentManagementService {
    repository: Arc<dyn DocumentMetadataRepository>,
    content_store: Arc<dyn ContentStore>,
    validator: ContentRepositoryValidator,
}

impl DocumentManagementService {
    pub fn new(
        repository: Arc<dyn DocumentMetadataRepository>,
        content_store: Arc<dyn ContentStore>,
    ) -> Self {
        Self {
            repository,
            content_store,
            validator: ContentRepositoryValidator::default(),
        }
    }

    pub fn with_validator(
        repository: Arc<dyn DocumentMetadataRepository>,
        content_store: Arc<dyn ContentStore>,
        validator: ContentRepositoryValidator,
    ) -> Self {
        Self {
            repository,
            content_store,
            validator,
        }
    }

    /// Creates an in-memory [`DocumentManagementService`] suitable for unit and integration testing.
    pub fn in_memory() -> Self {
        Self::new(
            Arc::new(InMemoryDocumentRepository::new()),
            Arc::new(InMemoryContentStore::new()),
        )
    }

    pub fn repository(&self) -> &Arc<dyn DocumentMetadataRepository> {
        &self.repository
    }

    pub fn content_store(&self) -> &Arc<dyn ContentStore> {
        &self.content_store
    }

    pub fn validator(&self) -> &ContentRepositoryValidator {
        &self.validator
    }

    /// Lists all documents for `(entity_type, entity_id)` (`DocumentReadPlatformServiceImpl.retrieveAllDocuments`).
    pub async fn retrieve_all_documents(
        &self,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<Vec<DocumentData>, FineractError> {
        let parent = DocumentParentEntityType::parse(entity_type)?;
        let docs = self
            .repository
            .list_documents(parent.as_str(), entity_id)
            .await?;
        Ok(docs.into_iter().map(DocumentData::from).collect())
    }

    /// Retrieves a single document metadata record (`DocumentReadPlatformServiceImpl.retrieveDocument`).
    pub async fn retrieve_document(
        &self,
        entity_type: &str,
        entity_id: i64,
        document_id: i64,
    ) -> Result<DocumentData, FineractError> {
        let parent = DocumentParentEntityType::parse(entity_type)?;
        let doc = self
            .repository
            .find_document(parent.as_str(), entity_id, document_id)
            .await?
            .ok_or_else(|| {
                ContentRepositoryValidator::document_not_found(
                    parent.as_str(),
                    entity_id,
                    document_id,
                )
            })?;
        Ok(doc.to_data())
    }

    /// Retrieves the binary content of a document (`DocumentReadPlatformServiceImpl.retrieveDocumentContent`).
    pub async fn retrieve_document_content(
        &self,
        entity_type: &str,
        entity_id: i64,
        document_id: i64,
    ) -> Result<DocumentContent, FineractError> {
        let parent = DocumentParentEntityType::parse(entity_type)?;
        let doc = self
            .repository
            .find_document(parent.as_str(), entity_id, document_id)
            .await?
            .ok_or_else(|| {
                ContentRepositoryValidator::document_not_found(
                    parent.as_str(),
                    entity_id,
                    document_id,
                )
            })?;
        let bytes = self.content_store.read_file(&doc.location).await?;
        let format = doc.file_name.rsplit('.').next().map(str::to_owned);
        Ok(DocumentContent {
            display_name: doc.name,
            file_name: doc.file_name,
            content_type: doc.content_type,
            format,
            size: bytes.len() as i64,
            bytes,
        })
    }

    /// Creates a new document and uploads its binary content (`DocumentWritePlatformServiceImpl.createDocument`).
    pub async fn create_document(
        &self,
        request: DocumentCreateRequest,
    ) -> Result<DocumentData, FineractError> {
        let parent = DocumentParentEntityType::parse(&request.entity_type)?;
        let clean_file_name = self.validator.validate_file_name(&request.file_name)?;
        let size = request.content.len() as i64;
        self.validator.validate_file_size(size)?;

        let path = ContentRepositoryValidator::build_entity_storage_path(
            DOCUMENTS_STORE_PREFIX,
            parent.as_str(),
            request.entity_id,
            &clean_file_name,
        )?;

        let stored_location = self.content_store.save_file(&path, &request.content).await?;
        let resolved_name = request
            .name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(&clean_file_name)
            .to_string();
        let resolved_type = request
            .content_type
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| {
                ContentRepositoryValidator::detect_mime_type_from_file_name(&clean_file_name)
                    .to_string()
            });

        let doc = Document {
            id: 0,
            parent_entity_type: parent.as_str().to_string(),
            parent_entity_id: request.entity_id,
            name: resolved_name,
            file_name: clean_file_name,
            size,
            content_type: resolved_type,
            description: request.description,
            location: stored_location,
            storage_type: self.content_store.storage_type(),
        };

        let saved = self.repository.insert_document(doc).await?;
        Ok(saved.to_data())
    }

    /// Updates an existing document's metadata and/or binary content (`DocumentWritePlatformServiceImpl.updateDocument`).
    pub async fn update_document(
        &self,
        request: DocumentUpdateRequest,
    ) -> Result<(DocumentData, BTreeMap<String, serde_json::Value>), FineractError> {
        let parent = DocumentParentEntityType::parse(&request.entity_type)?;
        let mut doc = self
            .repository
            .find_document(parent.as_str(), request.entity_id, request.id)
            .await?
            .ok_or_else(|| {
                ContentRepositoryValidator::document_not_found(
                    parent.as_str(),
                    request.entity_id,
                    request.id,
                )
            })?;

        let mut changes = BTreeMap::new();
        doc.storage_type = self.content_store.storage_type();

        if let Some(ref new_file_name) = request.file_name {
            let clean_file_name = self.validator.validate_file_name(new_file_name)?;
            if let Some(ref bytes) = request.content {
                let new_size = bytes.len() as i64;
                self.validator.validate_file_size(new_size)?;
                let new_path = ContentRepositoryValidator::build_entity_storage_path(
                    DOCUMENTS_STORE_PREFIX,
                    parent.as_str(),
                    request.entity_id,
                    &clean_file_name,
                )?;
                if !doc.location.eq_ignore_ascii_case(&new_path) {
                    let _ = self.content_store.delete_file(&doc.location).await;
                }
                let stored_path = self.content_store.save_file(&new_path, bytes).await?;
                if doc.file_name != clean_file_name {
                    changes.insert(
                        "fileName".to_string(),
                        serde_json::Value::String(clean_file_name.clone()),
                    );
                    doc.file_name = clean_file_name;
                }
                if doc.location != stored_path {
                    changes.insert(
                        "location".to_string(),
                        serde_json::Value::String(stored_path.clone()),
                    );
                    doc.location = stored_path;
                }
                if doc.size != new_size {
                    changes.insert("size".to_string(), serde_json::json!(new_size));
                    doc.size = new_size;
                }
            }
        } else if let Some(ref bytes) = request.content {
            let new_size = bytes.len() as i64;
            self.validator.validate_file_size(new_size)?;
            self.content_store.save_file(&doc.location, bytes).await?;
            if doc.size != new_size {
                changes.insert("size".to_string(), serde_json::json!(new_size));
                doc.size = new_size;
            }
        }

        if let Some(ref new_name) = request.name {
            let trimmed = new_name.trim();
            if !trimmed.is_empty() && !doc.name.eq_ignore_ascii_case(trimmed) {
                doc.name = trimmed.to_string();
                changes.insert(
                    "name".to_string(),
                    serde_json::Value::String(doc.name.clone()),
                );
            }
        }

        if let Some(ref new_desc) = request.description {
            let trimmed = new_desc.trim();
            if !trimmed.is_empty()
                && doc.description.as_deref().map_or(true, |d| !d.eq_ignore_ascii_case(trimmed))
            {
                doc.description = Some(trimmed.to_string());
                changes.insert(
                    "description".to_string(),
                    serde_json::Value::String(trimmed.to_string()),
                );
            }
        }

        if let Some(ref new_type) = request.content_type {
            let trimmed = new_type.trim();
            if !trimmed.is_empty() && !doc.content_type.eq_ignore_ascii_case(trimmed) {
                doc.content_type = trimmed.to_string();
                changes.insert(
                    "type".to_string(),
                    serde_json::Value::String(doc.content_type.clone()),
                );
            }
        }

        let updated = self.repository.update_document(doc).await?;
        Ok((updated.to_data(), changes))
    }

    /// Deletes a document from both the [`ContentStore`] and [`DocumentMetadataRepository`]
    /// (`DocumentWritePlatformServiceImpl.deleteDocument`).
    pub async fn delete_document(
        &self,
        entity_type: &str,
        entity_id: i64,
        document_id: i64,
    ) -> Result<i64, FineractError> {
        let parent = DocumentParentEntityType::parse(entity_type)?;
        let doc = self
            .repository
            .find_document(parent.as_str(), entity_id, document_id)
            .await?
            .ok_or_else(|| {
                ContentRepositoryValidator::document_not_found(
                    parent.as_str(),
                    entity_id,
                    document_id,
                )
            })?;

        self.content_store.delete_file(&doc.location).await?;
        self.repository
            .delete_document(parent.as_str(), entity_id, document_id)
            .await?;

        Ok(doc.id)
    }

    /// Creates or replaces an entity's profile image from a Base64 Data URI
    /// (`ImagesApiResource.createImage` + `ImageWritePlatformServiceImpl.createImage`).
    pub async fn save_image_from_data_uri(
        &self,
        entity_type: &str,
        entity_id: i64,
        data_uri: &str,
    ) -> Result<ImageData, FineractError> {
        let decoded = self.validator.decode_base64_data_uri(data_uri)?;
        let generated_file_name = format!("{}.{}", Uuid::new_v4(), decoded.extension);
        self.save_image_bytes(
            entity_type,
            entity_id,
            &generated_file_name,
            &decoded.mime_type,
            &decoded.bytes,
        )
        .await
    }

    /// Creates or replaces an entity's profile image from raw image bytes.
    pub async fn save_image_bytes(
        &self,
        entity_type: &str,
        entity_id: i64,
        file_name: &str,
        mime_type: &str,
        bytes: &[u8],
    ) -> Result<ImageData, FineractError> {
        let normalized_entity = Self::normalize_image_entity_type(entity_type)?;
        let clean_file_name = self.validator.validate_file_name(file_name)?;
        self.validator.validate_image_size(bytes.len() as i64)?;

        let mime_lower = mime_type.trim().to_ascii_lowercase();
        if !ContentRepositoryValidator::ALLOWED_IMAGE_MIME_TYPES.contains(&mime_lower.as_str()) {
            return Err(ContentRepositoryValidator::content_policy_error(format!(
                "Detected mime type {mime_type} for filename {clean_file_name} not allowed!"
            )));
        }

        // Delete previous image file if one exists (matching ImageWritePlatformServiceImpl.java line 95)
        if let Some(existing) = self
            .repository
            .find_image_by_entity(normalized_entity, entity_id)
            .await?
        {
            let _ = self.content_store.delete_file(&existing.location).await;
        }

        let path = ContentRepositoryValidator::build_entity_storage_path(
            IMAGES_STORE_PREFIX,
            normalized_entity,
            entity_id,
            &clean_file_name,
        )?;
        let stored_location = self.content_store.save_file(&path, bytes).await?;

        let image = ImageData {
            id: 0,
            location: stored_location,
            storage_type: self.content_store.storage_type(),
            entity_type: normalized_entity.to_string(),
            entity_id,
        };
        self.repository.upsert_image(image).await
    }

    /// Retrieves an entity's profile image and encodes it as a Base64 Data URI
    /// (`ImagesApiResource.retrieveImage`).
    pub async fn retrieve_image_as_data_uri(
        &self,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<String, FineractError> {
        let content = self.retrieve_image_content(entity_type, entity_id).await?;
        Ok(ContentRepositoryValidator::encode_base64_data_uri(
            &content.content_type,
            &content.bytes,
        ))
    }

    /// Retrieves an entity's raw profile image [`DocumentContent`] (`ImageReadPlatformServiceImpl.retrieveImage`).
    pub async fn retrieve_image_content(
        &self,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<DocumentContent, FineractError> {
        let normalized_entity = Self::normalize_image_entity_type(entity_type)?;
        let image = self
            .repository
            .find_image_by_entity(normalized_entity, entity_id)
            .await?
            .ok_or_else(|| {
                ContentRepositoryValidator::document_not_found(normalized_entity, entity_id, -1)
            })?;

        let bytes = self.content_store.read_file(&image.location).await?;
        let file_name = image.file_name().to_string();
        let format = file_name.rsplit('.').next().map(str::to_owned);
        let content_type =
            ContentRepositoryValidator::detect_mime_type_from_file_name(&file_name).to_string();

        Ok(DocumentContent {
            display_name: format!("{normalized_entity}_{entity_id}"),
            file_name,
            content_type,
            format,
            size: bytes.len() as i64,
            bytes,
        })
    }

    /// Deletes an entity's profile image (`ImageWritePlatformServiceImpl.deleteImage`).
    pub async fn delete_image(
        &self,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<i64, FineractError> {
        let normalized_entity = Self::normalize_image_entity_type(entity_type)?;
        let deleted = self
            .repository
            .delete_image(normalized_entity, entity_id)
            .await?
            .ok_or_else(|| {
                ContentRepositoryValidator::document_not_found(normalized_entity, entity_id, -1)
            })?;

        self.content_store.delete_file(&deleted.location).await?;
        Ok(entity_id)
    }

    fn normalize_image_entity_type(entity_type: &str) -> Result<&'static str, FineractError> {
        let parent = DocumentParentEntityType::parse(entity_type)?;
        match parent {
            DocumentParentEntityType::Clients => Ok("clients"),
            DocumentParentEntityType::Staff => Ok("staff"),
            _ => Err(FineractError::DataValidation(
                PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.image.entityType.invalid",
                    format!(
                        "Images are only supported for 'clients' and 'staff', got '{}'",
                        parent.as_str()
                    ),
                    DOCUMENT_API_PARAM_ENTITY_TYPE,
                    vec![ApiErrorArg::from(parent.as_str())],
                ),
            )),
        }
    }
}

fn apply_entity_audit_ids(
    builder: CommandProcessingResultBuilder,
    entity_type: &str,
    entity_id: i64,
) -> CommandProcessingResultBuilder {
    match DocumentParentEntityType::parse(entity_type) {
        Ok(DocumentParentEntityType::Clients) => builder.with_client_id(entity_id),
        Ok(DocumentParentEntityType::Loans) => builder.with_loan_id(entity_id),
        Ok(DocumentParentEntityType::Savings) => builder.with_savings_id(entity_id),
        Ok(DocumentParentEntityType::Groups) => builder.with_group_id(entity_id),
        _ => builder,
    }
}

/// CQRS command handler for `CREATE DOCUMENT` (`DocumentCreateCommandHandler.java`).
pub struct DocumentCreateCommandHandler {
    service: Arc<DocumentManagementService>,
}

/// Alias for [`DocumentCreateCommandHandler`].
pub type CreateDocumentCommandHandler = DocumentCreateCommandHandler;

impl DocumentCreateCommandHandler {
    pub fn new(service: Arc<DocumentManagementService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl CommandHandler for DocumentCreateCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let entity_type = command
            .string_value_of_parameter_named("entityType")
            .or_else(|| command.string_value_of_parameter_named("parentEntityType"))
            .unwrap_or_else(|| "clients".to_string());
        let entity_id = command
            .long_value_of_parameter_named("entityId")?
            .or_else(|| command.entity_id)
            .unwrap_or(0);
        let file_name = command
            .string_value_of_parameter_named("fileName")
            .unwrap_or_default();
        let name = command.string_value_of_parameter_named("name");
        let description = command.string_value_of_parameter_named("description");
        let content_type = command
            .string_value_of_parameter_named("contentType")
            .or_else(|| command.string_value_of_parameter_named("type"));

        let content = if let Some(b64) = command.string_value_of_parameter_named("contentBase64") {
            decode_base64(&b64)?
        } else if let Some(raw) = command.string_value_of_parameter_named("content") {
            raw.into_bytes()
        } else {
            Vec::new()
        };

        let created = self
            .service
            .create_document(DocumentCreateRequest {
                entity_type: entity_type.clone(),
                entity_id,
                name,
                file_name,
                content_type,
                description,
                content,
            })
            .await?;

        let builder = CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_resource_id(created.id)
            .with_sub_resource_id(created.id);
        Ok(apply_entity_audit_ids(builder, &entity_type, entity_id).build())
    }
}

/// CQRS command handler for `UPDATE DOCUMENT` (`DocumentUpdateCommandHandler.java`).
pub struct DocumentUpdateCommandHandler {
    service: Arc<DocumentManagementService>,
}

/// Alias for [`DocumentUpdateCommandHandler`].
pub type UpdateDocumentCommandHandler = DocumentUpdateCommandHandler;

impl DocumentUpdateCommandHandler {
    pub fn new(service: Arc<DocumentManagementService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl CommandHandler for DocumentUpdateCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let entity_type = command
            .string_value_of_parameter_named("entityType")
            .or_else(|| command.string_value_of_parameter_named("parentEntityType"))
            .unwrap_or_else(|| "clients".to_string());
        let entity_id = command
            .long_value_of_parameter_named("entityId")?
            .or_else(|| command.entity_id)
            .unwrap_or(0);
        let document_id = command
            .long_value_of_parameter_named("documentId")?
            .or(command.subresource_id)
            .or(command.entity_id)
            .unwrap_or(0);

        let name = command.string_value_of_parameter_named("name");
        let description = command.string_value_of_parameter_named("description");
        let file_name = command.string_value_of_parameter_named("fileName");
        let content_type = command
            .string_value_of_parameter_named("contentType")
            .or_else(|| command.string_value_of_parameter_named("type"));

        let content = if let Some(b64) = command.string_value_of_parameter_named("contentBase64") {
            Some(decode_base64(&b64)?)
        } else {
            command
                .string_value_of_parameter_named("content")
                .map(String::into_bytes)
        };

        let (updated, changes) = self
            .service
            .update_document(DocumentUpdateRequest {
                id: document_id,
                entity_type: entity_type.clone(),
                entity_id,
                name,
                file_name,
                content_type,
                description,
                content,
            })
            .await?;

        let builder = CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_resource_id(updated.id)
            .with_sub_resource_id(updated.id)
            .with_changes(changes);
        Ok(apply_entity_audit_ids(builder, &entity_type, entity_id).build())
    }
}

/// CQRS command handler for `DELETE DOCUMENT` (`DocumentDeleteCommandHandler.java`).
pub struct DocumentDeleteCommandHandler {
    service: Arc<DocumentManagementService>,
}

/// Alias for [`DocumentDeleteCommandHandler`].
pub type DeleteDocumentCommandHandler = DocumentDeleteCommandHandler;

impl DocumentDeleteCommandHandler {
    pub fn new(service: Arc<DocumentManagementService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl CommandHandler for DocumentDeleteCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let entity_type = command
            .string_value_of_parameter_named("entityType")
            .or_else(|| command.string_value_of_parameter_named("parentEntityType"))
            .unwrap_or_else(|| "clients".to_string());
        let entity_id = command
            .long_value_of_parameter_named("entityId")?
            .or_else(|| command.entity_id)
            .unwrap_or(0);
        let document_id = command
            .long_value_of_parameter_named("documentId")?
            .or(command.subresource_id)
            .or(command.entity_id)
            .unwrap_or(0);

        let deleted_id = self
            .service
            .delete_document(&entity_type, entity_id, document_id)
            .await?;

        let builder = CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_resource_id(deleted_id)
            .with_sub_resource_id(deleted_id);
        Ok(apply_entity_audit_ids(builder, &entity_type, entity_id).build())
    }
}

/// CQRS command handler for `CREATE IMAGE` and `UPDATE IMAGE` (`ImageCreateCommandHandler.java`).
pub struct ImageCreateCommandHandler {
    service: Arc<DocumentManagementService>,
}

/// Alias for [`ImageCreateCommandHandler`].
pub type CreateImageCommandHandler = ImageCreateCommandHandler;
/// Alias for [`ImageCreateCommandHandler`].
pub type ImageUpdateCommandHandler = ImageCreateCommandHandler;

impl ImageCreateCommandHandler {
    pub fn new(service: Arc<DocumentManagementService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl CommandHandler for ImageCreateCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let entity_type = command
            .string_value_of_parameter_named("entityType")
            .unwrap_or_else(|| DEFAULT_IMAGE_ENTITY_TYPE.to_string());
        let entity_id = command
            .long_value_of_parameter_named("entityId")?
            .or_else(|| command.entity_id)
            .unwrap_or(0);

        let saved = if let Some(data_uri) = command
            .string_value_of_parameter_named("dataUri")
            .or_else(|| command.string_value_of_parameter_named("image"))
        {
            self.service
                .save_image_from_data_uri(&entity_type, entity_id, &data_uri)
                .await?
        } else {
            let file_name = command
                .string_value_of_parameter_named("fileName")
                .unwrap_or_else(|| format!("{}{}", Uuid::new_v4(), DEFAULT_IMAGE_EXTENSION));
            let content_type = command
                .string_value_of_parameter_named("contentType")
                .or_else(|| command.string_value_of_parameter_named("type"))
                .unwrap_or_else(|| {
                    ContentRepositoryValidator::detect_mime_type_from_file_name(&file_name)
                        .to_string()
                });
            let bytes = if let Some(b64) = command.string_value_of_parameter_named("contentBase64")
            {
                decode_base64(&b64)?
            } else {
                Vec::new()
            };
            self.service
                .save_image_bytes(&entity_type, entity_id, &file_name, &content_type, &bytes)
                .await?
        };

        let builder = CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_resource_id(saved.id);
        Ok(apply_entity_audit_ids(builder, &entity_type, entity_id).build())
    }
}

/// CQRS command handler for `DELETE IMAGE` (`ImageDeleteCommandHandler.java`).
pub struct ImageDeleteCommandHandler {
    service: Arc<DocumentManagementService>,
}

/// Alias for [`ImageDeleteCommandHandler`].
pub type DeleteImageCommandHandler = ImageDeleteCommandHandler;

impl ImageDeleteCommandHandler {
    pub fn new(service: Arc<DocumentManagementService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl CommandHandler for ImageDeleteCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let entity_type = command
            .string_value_of_parameter_named("entityType")
            .unwrap_or_else(|| DEFAULT_IMAGE_ENTITY_TYPE.to_string());
        let entity_id = command
            .long_value_of_parameter_named("entityId")?
            .or_else(|| command.entity_id)
            .unwrap_or(0);

        let resource_id = self.service.delete_image(&entity_type, entity_id).await?;
        let builder = CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_resource_id(resource_id);
        Ok(apply_entity_audit_ids(builder, &entity_type, entity_id).build())
    }
}

/// Registers all document and image command handlers (`CREATE/UPDATE/DELETE DOCUMENT` and
/// `CREATE/UPDATE/DELETE IMAGE`) onto a [`CommandPipeline`].
pub fn register_document_command_handlers(
    pipeline: &CommandPipeline,
    service: Arc<DocumentManagementService>,
) {
    pipeline.register_handler(
        "CREATE",
        "DOCUMENT",
        Arc::new(DocumentCreateCommandHandler::new(Arc::clone(&service))),
    );
    pipeline.register_handler(
        "UPDATE",
        "DOCUMENT",
        Arc::new(DocumentUpdateCommandHandler::new(Arc::clone(&service))),
    );
    pipeline.register_handler(
        "DELETE",
        "DOCUMENT",
        Arc::new(DocumentDeleteCommandHandler::new(Arc::clone(&service))),
    );
    pipeline.register_handler(
        "CREATE",
        "IMAGE",
        Arc::new(ImageCreateCommandHandler::new(Arc::clone(&service))),
    );
    pipeline.register_handler(
        "UPDATE",
        "IMAGE",
        Arc::new(ImageCreateCommandHandler::new(Arc::clone(&service))),
    );
    pipeline.register_handler(
        "DELETE",
        "IMAGE",
        Arc::new(ImageDeleteCommandHandler::new(service)),
    );
}

// =============================================================================
// UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use fineract_command_rs::{CommandWrapperBuilder, InMemoryCommandSourceStore, MakerCheckerPolicy};

    #[test]
    fn storage_type_discriminants_match_java_enum() {
        assert_eq!(StorageType::FileSystem.value(), 1);
        assert_eq!(StorageType::S3.value(), 2);
        assert_eq!(StorageType::from_int(1), Some(StorageType::FileSystem));
        assert_eq!(StorageType::from_int(2), Some(StorageType::S3));
        assert_eq!(StorageType::from_int(99), None);
        assert!(StorageType::FileSystem.is_file_system());
        assert!(StorageType::S3.is_s3());
    }

    #[test]
    fn document_parent_entity_types_parse_and_format() {
        let cases = [
            ("clients", DocumentParentEntityType::Clients, true),
            ("STAFF", DocumentParentEntityType::Staff, true),
            ("loans", DocumentParentEntityType::Loans, false),
            ("savings", DocumentParentEntityType::Savings, false),
            (
                "client_identifiers",
                DocumentParentEntityType::ClientIdentifiers,
                false,
            ),
            ("groups", DocumentParentEntityType::Groups, false),
            ("import", DocumentParentEntityType::Import, false),
        ];

        for (raw, expected, supports_img) in cases {
            let parsed = DocumentParentEntityType::parse(raw).expect("valid entity type");
            assert_eq!(parsed, expected);
            assert_eq!(parsed.supports_images(), supports_img);
        }

        assert!(DocumentParentEntityType::parse("unknown_entity").is_err());
    }

    #[test]
    fn path_traversal_protection_rejects_sibling_and_parent_escapes() {
        let validator = ContentRepositoryValidator::default();

        // Exact attack vectors from TraversalContentPolicyTest.java plus Windows backslash and null-byte attacks
        let malicious_paths = [
            "/tmp/../17/michael.vorburger-crepes.jpg",
            "../17/michael.vorburger-crepes.jpg",
            "/tmp/..//17//michael.vorburger-crepes.jpg",
            "/tmp/..//17/michael.vorburger-crepes.jpg",
            "..//17/michael.vorburger-crepes.jpg",
            "../../../../../../../../../../tmp/michael.vorburger-crepes.jpg",
            "/tmp/../../../../../../../../../../tmp/image-text-wrong-content.jsp",
            "../../../../../../../../../../tmp/image-text-wrong-content.jsp",
            "/test/../tmp/image-text-wrong-content.jsp",
            "documents\\..\\secret.pdf",
            "documents/clients/1/../../etc/passwd",
            "/etc/passwd",
            "documents/clients/1/file\0.pdf",
        ];

        for bad_path in malicious_paths {
            let err = validator
                .validate_path(bad_path)
                .expect_err(&format!("Path traversal should be rejected: {bad_path}"));
            assert_eq!(err.http_status_code(), 400);
        }

        let malicious_filenames = [
            "../secret.pdf",
            "sub/file.pdf",
            "sub\\file.pdf",
            "..",
            ".",
            "",
            "   ",
        ];
        for bad_name in malicious_filenames {
            assert!(
                ContentRepositoryValidator::sanitize_file_name(bad_name).is_err(),
                "Filename should be rejected: {bad_name}"
            );
        }

        // Valid filename and storage path succeed
        assert_eq!(
            ContentRepositoryValidator::sanitize_file_name("passport_scan-2026.pdf").unwrap(),
            "passport_scan-2026.pdf"
        );
        assert_eq!(
            validator
                .validate_path("documents/clients/7/passport_scan-2026.pdf")
                .unwrap(),
            "documents/clients/7/passport_scan-2026.pdf"
        );
    }

    #[test]
    fn base64_data_uri_parsing_supports_png_jpeg_gif_and_rejects_invalid() {
        let validator = ContentRepositoryValidator::default();

        // 1x1 PNG in Base64
        let png_uri = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
        let parsed_png = validator.decode_base64_data_uri(png_uri).expect("valid png");
        assert_eq!(parsed_png.mime_type, "image/png");
        assert_eq!(parsed_png.extension, "png");
        assert_eq!(&parsed_png.bytes[0..4], &[0x89, b'P', b'N', b'G']);

        // JPEG Data URI
        let jpeg_uri = "data:image/jpeg;base64,/9j/4AAQSkZJRgABAQEASABIAAD/";
        let parsed_jpg = validator
            .decode_base64_data_uri(jpeg_uri)
            .expect("valid jpeg");
        assert_eq!(parsed_jpg.mime_type, "image/jpeg");
        assert_eq!(parsed_jpg.extension, "jpg");
        assert_eq!(&parsed_jpg.bytes[0..3], &[0xFF, 0xD8, 0xFF]);

        // GIF Data URI
        let gif_uri =
            "data:image/gif;base64,R0lGODlhAQABAIAAAP///wAAACH5BAEAAAAALAAAAAABAAEAAAICRAEAOw==";
        let parsed_gif = validator.decode_base64_data_uri(gif_uri).expect("valid gif");
        assert_eq!(parsed_gif.mime_type, "image/gif");
        assert_eq!(parsed_gif.extension, "gif");
        assert_eq!(&parsed_gif.bytes[0..6], b"GIF89a");

        // Round-trip encode -> decode
        let reencoded =
            ContentRepositoryValidator::encode_base64_data_uri("image/gif", &parsed_gif.bytes);
        assert_eq!(reencoded, gif_uri);

        // Disallowed MIME type in Data URI
        assert!(validator
            .decode_base64_data_uri("data:text/html;base64,PGh0bWw+PC9odG1sPg==")
            .is_err());

        // Missing ;base64
        assert!(validator
            .decode_base64_data_uri("data:image/png,iVBORw0KGgo=")
            .is_err());

        // Malformed Base64 characters
        assert!(validator
            .decode_base64_data_uri("data:image/png;base64,$$$invalid$$$")
            .is_err());
    }

    #[test]
    fn max_file_size_validation_enforces_positive_and_upper_bounds() {
        let validator = ContentRepositoryValidator::new(1024);

        assert!(validator.validate_file_size(0).is_err());
        assert!(validator.validate_file_size(-5).is_err());
        assert!(validator.validate_file_size(1).is_ok());
        assert!(validator.validate_file_size(1024).is_ok());
        assert!(validator.validate_file_size(1025).is_err());

        // Oversized Base64 image rejected when exceeding max_image_size
        let tiny_validator = ContentRepositoryValidator::new(10);
        let gif_uri =
            "data:image/gif;base64,R0lGODlhAQABAIAAAP///wAAACH5BAEAAAAALAAAAAABAAEAAAICRAEAOw==";
        assert!(tiny_validator.decode_base64_data_uri(gif_uri).is_err());
    }

    #[tokio::test]
    async fn local_filesystem_content_store_lifecycle_and_traversal_defense() {
        let temp_root = std::env::temp_dir().join(format!("fineract_doc_test_{}", Uuid::new_v4()));
        let store = LocalFileSystemContentStore::new(&temp_root);

        let location = "documents/clients/42/loan_agreement.pdf";
        let payload = b"%PDF-1.7 sample loan agreement bytes";

        let saved_loc = store
            .save_file(location, payload)
            .await
            .expect("save_file succeeds");
        assert_eq!(saved_loc, location);

        let read_back = store
            .read_file(&saved_loc)
            .await
            .expect("read_file succeeds");
        assert_eq!(read_back, payload);

        // Traversal attempt against LocalFileSystemContentStore is rejected before touching disk
        assert!(store
            .save_file("../escaped.pdf", b"malicious")
            .await
            .is_err());

        store
            .delete_file(&saved_loc)
            .await
            .expect("delete_file succeeds");
        assert!(store.read_file(&saved_loc).await.is_err());

        let _ = tokio::fs::remove_dir_all(&temp_root).await;
    }

    #[test]
    fn dual_db_sql_queries_support_postgres_and_mysql() {
        let pg_insert = DualDbDocumentRepository::insert_document_sql(SqlDialect::Postgres);
        assert!(pg_insert.contains("$1") && pg_insert.contains("$10"));
        assert!(!pg_insert.contains('?'));

        let mysql_insert = DualDbDocumentRepository::insert_document_sql(SqlDialect::MySql);
        assert!(mysql_insert.contains('?'));
        assert!(!mysql_insert.contains("$1"));

        let pg_client_img =
            DualDbDocumentRepository::select_entity_image_sql("clients", SqlDialect::Postgres)
                .unwrap();
        assert!(pg_client_img.contains("INNER JOIN m_client e ON e.image_id = i.id WHERE e.id = $1"));

        let mysql_staff_img =
            DualDbDocumentRepository::select_entity_image_sql("staff", SqlDialect::MySql).unwrap();
        assert!(mysql_staff_img.contains("INNER JOIN m_staff e ON e.image_id = i.id WHERE e.id = ?"));
    }

    #[tokio::test]
    async fn end_to_end_document_and_image_lifecycle_via_command_pipeline() {
        let service = Arc::new(DocumentManagementService::in_memory());
        let cmd_store = Arc::new(InMemoryCommandSourceStore::new());
        let pipeline = CommandPipeline::new(cmd_store, MakerCheckerPolicy::new(false));
        register_document_command_handlers(&pipeline, Arc::clone(&service));

        let ctx = ExecutionContext::new(
            "default",
            NaiveDate::from_ymd_opt(2026, 10, 30).expect("valid date"),
        );

        // 1. Create Document via CommandPipeline
        let create_json = serde_json::json!({
            "entityType": "clients",
            "entityId": 7,
            "name": "National ID",
            "description": "Front and back scan",
            "fileName": "national_id.pdf",
            "contentType": "application/pdf",
            "contentBase64": encode_base64(b"PDF-CONTENT-V1")
        })
        .to_string();
        let create_wrapper = CommandWrapperBuilder::new()
            .with_action_name("CREATE")
            .with_entity_name("DOCUMENT")
            .with_entity_id(7)
            .with_json(create_json.clone())
            .build();
        let create_cmd = JsonCommand::from_json(create_json)
            .unwrap()
            .with_entity_id(7);
        let create_res = pipeline
            .execute(&ctx, &create_wrapper, &create_cmd, false)
            .await
            .expect("create document command succeeds");
        let doc_id = create_res.resource_id.expect("doc_id assigned");
        assert_eq!(create_res.client_id, Some(7));

        // 2. Verify document metadata and binary download
        let docs = service
            .retrieve_all_documents("clients", 7)
            .await
            .expect("list documents");
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].id, doc_id);
        assert_eq!(docs[0].name, "National ID");
        assert_eq!(docs[0].location, "documents/clients/7/national_id.pdf");

        let downloaded = service
            .retrieve_document_content("clients", 7, doc_id)
            .await
            .expect("download content");
        assert_eq!(downloaded.bytes, b"PDF-CONTENT-V1");

        // 3. Update Document via CommandPipeline
        let update_json = serde_json::json!({
            "entityType": "clients",
            "entityId": 7,
            "documentId": doc_id,
            "name": "Updated National ID",
            "description": "Verified scan",
            "fileName": "national_id_v2.pdf",
            "contentBase64": encode_base64(b"PDF-CONTENT-V2-LONGER")
        })
        .to_string();
        let update_wrapper = CommandWrapperBuilder::new()
            .with_action_name("UPDATE")
            .with_entity_name("DOCUMENT")
            .with_entity_id(7)
            .with_subentity_id(doc_id)
            .with_json(update_json.clone())
            .build();
        let update_cmd = JsonCommand::from_json(update_json)
            .unwrap()
            .with_entity_id(7)
            .with_subresource_id(doc_id);
        let update_res = pipeline
            .execute(&ctx, &update_wrapper, &update_cmd, false)
            .await
            .expect("update document command succeeds");
        assert!(update_res.has_changes());

        let updated_content = service
            .retrieve_document_content("clients", 7, doc_id)
            .await
            .expect("download updated content");
        assert_eq!(updated_content.bytes, b"PDF-CONTENT-V2-LONGER");
        assert_eq!(updated_content.file_name, "national_id_v2.pdf");

        // 4. Create & Retrieve Client Image via Data URI
        let gif_uri =
            "data:image/gif;base64,R0lGODlhAQABAIAAAP///wAAACH5BAEAAAAALAAAAAABAAEAAAICRAEAOw==";
        let img_json = serde_json::json!({
            "entityType": "clients",
            "entityId": 7,
            "dataUri": gif_uri
        })
        .to_string();
        let img_wrapper = CommandWrapperBuilder::new()
            .with_action_name("CREATE")
            .with_entity_name("IMAGE")
            .with_entity_id(7)
            .with_json(img_json.clone())
            .build();
        let img_cmd = JsonCommand::from_json(img_json).unwrap().with_entity_id(7);
        let img_res = pipeline
            .execute(&ctx, &img_wrapper, &img_cmd, false)
            .await
            .expect("create image command succeeds");
        assert!(img_res.resource_id.is_some());

        let retrieved_data_uri = service
            .retrieve_image_as_data_uri("clients", 7)
            .await
            .expect("retrieve image as data URI");
        assert_eq!(retrieved_data_uri, gif_uri);

        // 5. Delete Image & Document via CommandPipeline
        let del_img_json = serde_json::json!({
            "entityType": "clients",
            "entityId": 7
        })
        .to_string();
        let del_img_wrapper = CommandWrapperBuilder::new()
            .with_action_name("DELETE")
            .with_entity_name("IMAGE")
            .with_entity_id(7)
            .with_json(del_img_json.clone())
            .build();
        let del_img_cmd = JsonCommand::from_json(del_img_json)
            .unwrap()
            .with_entity_id(7);
        pipeline
            .execute(&ctx, &del_img_wrapper, &del_img_cmd, false)
            .await
            .expect("delete image succeeds");
        assert!(service.retrieve_image_as_data_uri("clients", 7).await.is_err());

        let del_doc_json = serde_json::json!({
            "entityType": "clients",
            "entityId": 7,
            "documentId": doc_id
        })
        .to_string();
        let del_doc_wrapper = CommandWrapperBuilder::new()
            .with_action_name("DELETE")
            .with_entity_name("DOCUMENT")
            .with_entity_id(7)
            .with_subentity_id(doc_id)
            .with_json(del_doc_json.clone())
            .build();
        let del_doc_cmd = JsonCommand::from_json(del_doc_json)
            .unwrap()
            .with_entity_id(7)
            .with_subresource_id(doc_id);
        pipeline
            .execute(&ctx, &del_doc_wrapper, &del_doc_cmd, false)
            .await
            .expect("delete document succeeds");

        let remaining = service.retrieve_all_documents("clients", 7).await.unwrap();
        assert!(remaining.is_empty());
    }
}
