# import the necessary packages
import argparse

import numpy as np
import cv2


def dhash(image, hashSize=8):
    # convert the image to grayscale
    gray = cv2.cvtColor(image, cv2.COLOR_BGR2GRAY)

    # resize the grayscale image, adding a single column (width) so we
    # can compute the horizontal gradient
    resized = cv2.resize(gray, (hashSize + 1, hashSize))

    # compute the (relative) horizontal gradient between adjacent
    # column pixels
    diff = resized[:, 1:] > resized[:, :-1]

    # show internals
    [print(resized[i, :], diff[i, :]) for i in range(0, hashSize)]

    # convert the difference image to a hash
    return sum([2**i for (i, v) in enumerate(diff.flatten()) if v])


def convert_hash(h):
    # convert the hash to NumPy's 64-bit float and then back to
    # Python's built in int
    return int(np.array(h, dtype="float64"))


def hamming(a, b):
    # compute and return the Hamming distance between the integers
    return bin(int(a) ^ int(b)).count("1")


ap = argparse.ArgumentParser()
ap.add_argument("-i", required=True, help="The image file to hash")

if __name__ == "__main__":
    args = ap.parse_args()
    # print(args)
    img = cv2.imread(args.i)
    hash = dhash(img)
    print(f"0x{hash:x}, decimal: {hash}")
