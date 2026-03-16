FROM node:22-alpine
WORKDIR /app
RUN echo '{"name":"backend","scripts":{"start":"node server.js"}}' > package.json
RUN echo 'const http = require("http"); const port = process.env.PORT || 8080; http.createServer((_, res) => { res.end("backend ok"); }).listen(port, () => console.log(`backend on ${port}`));' > server.js
EXPOSE 8080
CMD ["node", "server.js"]
