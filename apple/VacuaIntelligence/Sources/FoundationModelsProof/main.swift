import FoundationModels

// Compile proof: must fail compilation on any SDK lacking FoundationModels
if #available(macOS 26.0, *) {
    let model = SystemLanguageModel.default
    _ = model.availability
}
print("FoundationModels compile proof: OK")
