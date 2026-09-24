# A pipe that ends the pipeline leaves every element to fit whole.
ls
| to json [
    a
] e>|
ls
| to json [
    a
] o+e>|
