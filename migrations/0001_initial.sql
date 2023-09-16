CREATE TABLE image (
    mtime DATETIME NOT NULL, -- File mtime on disk
    url TEXT NOT NULL, -- The path on the app's side
    inserted DATETIME NOT NULL,

    PRIMARY KEY (mtime, url)
);

-- Image information calculated by the app

CREATE TABLE dhash (
    image_id INTEGER PRIMARY KEY,
    dhash INTEGER NOT NULL,
    inserted DATETIME NOT NULL

    -- TODO Explain why not!!!
    --FOREIGN KEY (image_id) REFERENCES image (
    --    rowid
    --) ON DELETE CASCADE ON UPDATE NO ACTION
);

CREATE TABLE thumbnail (
    image_id INTEGER PRIMARY KEY,
    base64 TEXT NOT NULL,
    inserted DATETIME NOT NULL,

    FOREIGN KEY (image_id) REFERENCES image (
        rowid
    ) ON DELETE CASCADE ON UPDATE NO ACTION
);
