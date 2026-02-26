# Artifact for Optimism in Equality Saturation

> [!NOTE]
> If you're cloning the [GitHub repository](https://github.com/RArbore/pldi26-artifact) directly, simply run `cargo test --release` to build and run the test suite, and take a look at [ARCHITECTURE.md](./ARCHITECTURE.md) to see how the project is structured.
> If you're an artifact reviewer starting from the Zenodo artifact, continue reading.

## Getting Started Guide

*Estimated time to complete this section: 5 minutes.*

### Step 1: Download Docker image and run script

Download the following files from the Zenodo archive into a new directory on your computer:

1. `opt-eqsat.tar.gz`: Docker image of software artifact.
2. `run.sh`: Script to run the software artifact inside the Docker image.
3. `expected_paper_example1_clang.S`: expected assembly output of Clang run on the first paper example.
4. `expected_paper_example2_clang.S`: expected assembly output of Clang run on the second paper example.
5. `expected_paper_example1_gcc.S`: expected assembly output of GCC run on the first paper example.
6. `expected_paper_example2_gcc.S`: expected assembly output of GCC run on the second paper example.

### Step 2: Install and set up Podman

Install the Docker alternative [Podman](https://podman.io). If this is your first time ever using Podman *and* you are not running Linux, you will need to set up a virtual machine by running the following commands:

1. `podman machine init`
2. `podman machine start`

### Step 3: Load the Docker image

Load the Docker image by running the following command from the directory you created in step 1:

1. `podman load -i opt-eqsat.tar.gz`

### Step 4: Run the examples from the paper

Run the following command from the directory you created in step 1:

1. `sh run.sh`

If everything is working correctly, you should see "All done!" appear in your shell. You should also see two tests run and complete successfully: `paper_example1` and `paper_example2`. This should take less than 10 seconds. This stresses the key element of our artifact, which is that the two example programs from the original paper submission are able to be precisely analyzed by our method.

## Step-by-Step Instructions

*Estimated time to complete this section: 5 minutes.*

Please ensure that `run.sh` from step 4 of the getting started guide has finished running. This should take less than 10 seconds.

### Step 1: Verify that the `paper_example1` and `paper_example2` tests passed.

In the output from `run.sh`, two lines should appear as follows (the order of the tests finishing does not matter and may differ depending on hardware):

```
test rewrites::tests::paper_example2 ... ok
test rewrites::tests::paper_example1 ... ok
```

This certifies the first claim made at the end of section 7.2 in the original paper, which is that our method can analyze the two example programs shown in figures 6 and 7 precisely enough (specifically, that the analysis finds that the returned outputs of both programs are constants).

### Step 2: Inspect the assembly outputs of Clang and GCC.

After `run.sh` has finished, four files should be present in the same directory as `run.sh` (the one created in step 1 of the getting started guide). These files are `paper_example1_clang.S`, `paper_example2_clang.S`, `paper_example1_gcc.S`, and `paper_example2_gcc.S`. Check that these files match the corresponding `expected_*.S` files, specifically:

1. `paper_example1_clang.S` should match `expected_paper_example1_clang.S`.
2. `paper_example2_clang.S` should match `expected_paper_example2_clang.S`.
3. `paper_example1_gcc.S` should match `expected_paper_example1_gcc.S`.
4. `paper_example2_gcc.S` should match `expected_paper_example2_gcc.S`.

This certifies the second claim made at the end of section 7.2 in the original paper, which is that Clang 21.1.0 and GCC 15.2 cannot analyze the two example programs shown in figures 6 and 7 well enough to eliminate the loops (the generated assembly includes a loop to compute the result, while both programs always return constant values that can be statically determined).

Thank you very much for your service as an artifact evaluator!

### Optional: Look at the `opt-eqsat` codebase.

If you would like to take a look at the implementation of `opt-eqsat`, please download and unzip the `code.zip` file from the Zenodo archive. Please refer to the `ARCHITECTURE.md` file to understand how the codebase is organized. 

Alternatively, if you would like to run more tests in the repository interactively, run `podman run -it --entrypoint /bin/bash opt-eqsat:latest` to load a shell inside the Docker container, and then `cargo test --release` to run the test suite except the torture tests (should finish in less than a minute) or `cargo test --release torture -- --ignored --nocapture` to run the torture generated test (will take at least a few minutes, possibly up to an hour or two depending on hardware - a progress indicator is printed regularly, showing how many generated programs have been tested so far).
