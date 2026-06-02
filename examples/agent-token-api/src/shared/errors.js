export class ApiError extends Error {
  constructor(statusCode, code, message, options = {}) {
    super(message);
    this.name = 'ApiError';
    this.statusCode = statusCode;
    this.code = code;
    this.expose = true;
    this.fields = options.fields;
  }
}
