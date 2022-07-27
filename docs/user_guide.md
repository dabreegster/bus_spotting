# User guide

The web version is deployed every time I push code. Since this software is
under rapid development, this means things might be broken. After the project
wraps up in August, things will calm down and I'll improve these instructions.

To use this tool, you need to:

1.  Create a .zip file with the raw input
2.  Load the file in the browser tool

## Creating the input file

The easiest method is to download the file shared privately. (It contains
non-public data, so only project stakeholders have access.)

You can create the zip archive yourself. It must contain at least one folder:

- `gtfs`, containing the 9 CSV files from `google_transit-02-2019` (from the
  Google Drive folder)

It can optionally contain two more folders:

- `avl`, containing the AVL file for one day
  - You can include multiple files, but only one will be imported right now
- `bil`, containing the BIL ticketing file for one day
  - You can include multiple files, but only one will be imported right now

## Importing the data

1.  Go to <https://dabreegster.github.io/bus_spotting>
2.  Click **Import data**
3.  Choose the `.zip` file from above
4.  Click **OK** and wait for the import. It takes about 10 seconds on my
    laptop for the sample .zip file shared.

Then you can use the app. I won't describe how to use it yet, since it's
changing constantly.

Note the import step is slow in the browser. There's also a "Load model" button
meant to avoid importing every time you use the app, but I'm having trouble
getting the browser to download large files after running the import. This'll
be fixed.
