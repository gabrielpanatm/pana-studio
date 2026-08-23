/** Pure request epoch used to reject responses from a replaced project session. */
export class VersioningSessionEpoch {
  private key = "";
  private serial = 0;

  synchronize(projectRoot: string, sessionId: string) {
    const nextKey = projectRoot && sessionId ? `${projectRoot}\u0000${sessionId}` : "";
    if (nextKey === this.key) return { changed: false, serial: this.serial } as const;
    this.key = nextKey;
    this.serial += 1;
    return { changed: true, serial: this.serial } as const;
  }

  nextRequest() {
    this.serial += 1;
    return this.serial;
  }

  current() {
    return this.serial;
  }

  isCurrent(serial: number) {
    return serial === this.serial;
  }
}
