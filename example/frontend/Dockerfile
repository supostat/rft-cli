FROM node:22-alpine
WORKDIR /app
RUN echo '{"name":"frontend","scripts":{"start":"node server.js"}}' > package.json
RUN echo 'const http = require("http"); const port = process.env.PORT || 3000; http.createServer((_, res) => { res.end("frontend ok"); }).listen(port, () => console.log(`frontend on ${port}`));' > server.js
EXPOSE 3000
CMD ["node", "server.js"]
