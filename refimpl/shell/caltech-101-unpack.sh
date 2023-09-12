TARGET_DIR="$(dirname $0)/tmp"
ARCHIVE_FILE="caltech-101.zip"
ARCHIVE_PATH="caltech-101/101_ObjectCategories.tar.gz"
IMAGE_DIR="101_ObjectCategories"

cd "$TARGET_DIR"
unzip "$ARCHIVE_FILE" "$ARCHIVE_PATH"
tar zxvf "$ARCHIVE_PATH"
mv "$IMAGE_DIR" ../..
cd -

# cleanup
rm -rf "$TARGET_DIR"
