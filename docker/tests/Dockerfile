FROM ubuntu:24.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
      dbus gnome-keyring libsecret-tools libglib2.0-bin python3 util-linux \
    && rm -rf /var/lib/apt/lists/* \
    && if ! getent passwd 1000 >/dev/null; then useradd --uid 1000 lettuce-test; fi \
    && install -d -o 1000 -g 0 /home/lettuce-test
COPY --chmod=755 docker/launch.sh /usr/local/bin/lettuceai
COPY --chmod=755 docker/keyring-session.sh /usr/local/lib/lettuceai/keyring-session
COPY --chmod=755 docker/container-state.py /usr/local/bin/lettuceai-container-status
COPY --chmod=755 docker/tests/application.sh /opt/lettuceai/bin/lettuceai
COPY --chmod=755 docker/tests/keyring.sh /test/keyring.sh
USER 1000
ENV HOME=/home/lettuce-test
CMD ["bash", "/test/keyring.sh"]
