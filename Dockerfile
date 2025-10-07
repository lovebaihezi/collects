# Use Fedora as the base image
FROM fedora:latest

# Set the working directory
WORKDIR /app



# Copy the binary from the release build
COPY target/release/collects-services .

# Set the command to run the service
CMD ["./collects-services"]
