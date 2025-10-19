# Use gcr.io/distroless/static-debian12 as the base image.
# This image is minimal and does not contain a shell or other utilities,
# which makes it more secure.
FROM gcr.io/distroless/static-debian12

# Set the working directory.
WORKDIR /app

# Copy the statically linked binary from the local build target directory.
# This binary must be built with the musl target before building the Docker image.
COPY target/x86_64-unknown-linux-musl/release/collects-services .

# Set the command to run the service.
CMD ["./collects-services"]
