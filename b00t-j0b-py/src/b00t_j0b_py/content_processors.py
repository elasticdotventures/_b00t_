"""Content processors for different file types (PDF, etc.)."""

import io
from typing import Dict, Any, Optional
from returns.result import Result, Success, Failure
import requests
try:
    import PyPDF2
except ImportError:
    PyPDF2 = None

from .config import config


class PDFProcessor:
    """Process PDF content."""
    
    def __init__(self):
        """Initialize PDF processor."""
        if PyPDF2 is None:
            raise ImportError("PyPDF2 not available - install with: uv add pypdf2")
    
    def can_process(self, content_type: str) -> bool:
        """Check if can process this content type."""
        return content_type.lower() in ["application/pdf", "application/x-pdf"]
    
    def process(self, content: bytes, url: str) -> Result[str, Exception]:
        """Extract text from PDF."""
        try:
            pdf_stream = io.BytesIO(content)
            pdf_reader = PyPDF2.PdfReader(pdf_stream)
            
            text_parts = [f"# PDF Content from {url}\n\n"]
            
            # Extract text from all pages
            for page_num, page in enumerate(pdf_reader.pages, 1):
                try:
                    page_text = page.extract_text()
                    if page_text.strip():
                        text_parts.append(f"## Page {page_num}\n\n{page_text}\n\n")
                except Exception as e:
                    text_parts.append(f"## Page {page_num}\n\n[Error extracting text: {e}]\n\n")
            
            return Success("".join(text_parts))
            
        except Exception as e:
            return Failure(e)


class AudioProcessor:
    """Process audio content (stub for future implementation)."""
    
    def can_process(self, content_type: str) -> bool:
        """Check if can process this content type."""
        audio_types = ["audio/mpeg", "audio/mp3", "audio/wav", "audio/ogg"]
        return content_type.lower() in audio_types
    
    def process(self, content: bytes, url: str) -> Result[str, Exception]:
        """Process audio content (placeholder)."""
        return Success(f"""# Audio Content from {url}

🎵 Audio content detected but processing not implemented yet.

**Content Type:** {content.decode('utf-8', errors='ignore')[:100] if len(content) < 1000 else 'Binary audio data'}
**Size:** {len(content)} bytes

*Future: Implement audio transcription using whisper or similar.*
""")


class ImageProcessor:
    """Process image content (stub for future OCR implementation)."""
    
    def can_process(self, content_type: str) -> bool:
        """Check if can process this content type."""
        image_types = ["image/jpeg", "image/jpg", "image/png", "image/gif", "image/webp"]
        return content_type.lower() in image_types
    
    def process(self, content: bytes, url: str) -> Result[str, Exception]:
        """Process image content (placeholder)."""
        return Success(f"""# Image Content from {url}

🖼️ Image content detected but OCR processing not implemented yet.

**Size:** {len(content)} bytes

*Future: Implement OCR using tesseract or similar.*
""")


class ContentProcessorRegistry:
    """Registry for content processors."""
    
    def __init__(self):
        """Initialize processor registry."""
        self._processors = []
        
        # Register built-in processors
        try:
            self.register(PDFProcessor())
        except ImportError:
            pass  # PDF processing not available
        
        self.register(AudioProcessor())
        self.register(ImageProcessor())
    
    def register(self, processor) -> None:
        """Register a processor."""
        self._processors.append(processor)
    
    def get_processor(self, content_type: str):
        """Get appropriate processor for content type."""
        for processor in self._processors:
            if processor.can_process(content_type):
                return processor
        return None
    
    def process_content(self, content: bytes, content_type: str, url: str) -> Result[str, Exception]:
        """Process content using appropriate processor."""
        processor = self.get_processor(content_type)
        if not processor:
            return Failure(ValueError(f"No processor available for content type: {content_type}"))
        
        return processor.process(content, url)


# Global registry instance
content_registry = ContentProcessorRegistry()