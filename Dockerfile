# Use gcr.io/distroless/static-debian12 as the base image.
# This image is minimal and does not contain a shell or other utilities,
# which makes it more secure.
FROM gcr.io/distroless/static-debian12

# Set the working directory.
WORKDIR /app

# Copy the statically linked binary from the local build target directory.
# This binary must be built with the musl target before building the Docker image.
COPY target/x86_64-unknown-linux-musl/release/collects-services .

# Default environment variable (can be overridden at runtime)
ENV ENV=prod
ENV PORT=3000

# Set the command to run the service.
# The service reads ENV and PORT from environment variables.
CMD ["./collects-services"]
