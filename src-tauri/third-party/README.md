# Phần mềm của bên khác trong HP Screen

HP Screen nhận chữ tiếng Việt bằng Tesseract OCR. Tesseract và các thư viện nó cần được biên dịch bằng [vcpkg](https://github.com/microsoft/vcpkg) rồi gộp tĩnh vào file chạy; dữ liệu tiếng Việt được gộp nguyên vẹn. Tất cả là công sức của các tác giả gốc, không phải của HP Media, và giữ nguyên giấy phép gốc. Thư mục này không gắn dòng bản quyền HP Media.

| Thành phần | Phiên bản | Giấy phép | Văn bản giấy phép |
| --- | --- | --- | --- |
| [Tesseract OCR](https://github.com/tesseract-ocr/tesseract) | 5.5.4 | Apache License 2.0 | `licenses/tesseract.txt` |
| [tessdata_fast](https://github.com/tesseract-ocr/tessdata_fast), `vie.traineddata` (dữ liệu tiếng Việt, không sửa) | nhánh `main` | Apache License 2.0 | `licenses/tessdata_fast.txt` |
| [Leptonica](http://www.leptonica.org/) | 1.87.0 | BSD 2 điều khoản | `licenses/leptonica.txt` |
| [libjpeg-turbo](https://libjpeg-turbo.org/) | 3.2.1 | IJG, BSD 3 điều khoản, zlib | `licenses/libjpeg-turbo.txt` |
| [libpng](http://www.libpng.org/) | 1.6.59 | giấy phép libpng | `licenses/libpng.txt` |
| [zlib](https://zlib.net/) | 1.4 | giấy phép zlib | `licenses/zlib.txt` |
| [LibTIFF](https://libtiff.gitlab.io/libtiff/) | 4.7.2 | giấy phép LibTIFF | `licenses/tiff.txt` |
| [XZ Utils / liblzma](https://tukaani.org/xz/) | 5.8.4 | BSD 0 điều khoản | `licenses/liblzma.txt` |
| [GIFLIB](https://giflib.sourceforge.net/) | 6.1.3 | MIT | `licenses/giflib.txt` |
| [libwebp](https://chromium.googlesource.com/webm/libwebp) | 1.6.0 | BSD 3 điều khoản | `licenses/libwebp.txt` |
| [OpenJPEG](https://www.openjpeg.org/) | 2.5.4 | BSD 2 điều khoản | `licenses/openjpeg.txt` |

`tessdata/vie.traineddata` là bản gốc từ tessdata_fast (SHA-256 `79df64caf7bcfb2a27df5042ecb6121e196eada34da774956995747636d5bfa1`).

Các văn bản giấy phép được cài cùng HP Screen, trong thư mục `third-party\licenses` cạnh file chạy.
