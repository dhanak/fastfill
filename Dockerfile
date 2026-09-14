FROM mcr.microsoft.com/dotnet/sdk:10.0.100-noble

WORKDIR /work

COPY Directory.Build.props Directory.Packages.props global.json ./
COPY src/FastFill.Core/FastFill.Core.csproj src/FastFill.Core/
COPY src/FastFill.Checks/FastFill.Checks.csproj src/FastFill.Checks/
RUN dotnet restore src/FastFill.Checks/FastFill.Checks.csproj

COPY src/FastFill.Core/ src/FastFill.Core/
COPY src/FastFill.Checks/ src/FastFill.Checks/
COPY tests/ tests/
RUN dotnet build src/FastFill.Checks/FastFill.Checks.csproj \
  -c Release --no-restore

ENTRYPOINT ["dotnet", \
  "src/FastFill.Checks/bin/Release/net10.0/FastFill.Checks.dll"]
