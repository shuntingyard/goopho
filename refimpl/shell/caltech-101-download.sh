TARGET_DIR="$(dirname $0)/tmp"
mkdir -p "$TARGET_DIR"
cd "$TARGET_DIR"
wget https://data.caltech.edu/records/mzrjq-6wc02/files/caltech-101.zip
cd -

# From pyimagesearch, no longer exists.
#wget http://www.vision.caltech.edu/Image_Datasets/Caltech101/101_ObjectCategories.tar.gz
