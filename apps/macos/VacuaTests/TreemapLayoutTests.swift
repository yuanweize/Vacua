import Testing
import Foundation
import CoreGraphics
@testable import Vacua

@Suite("Squarified Treemap Layout Invariants")
struct TreemapLayoutTests {

    @Test("Empty or zero bounds returns empty layout")
    func testEmptyBounds() {
        let items = [TreemapItem(id: "1", weight: 100)]
        let empty1 = SquarifiedTreemapLayout.layout(items: items, bounds: .zero)
        #expect(empty1.isEmpty)

        let empty2 = SquarifiedTreemapLayout.layout(items: [], bounds: CGRect(x: 0, y: 0, width: 800, height: 600))
        #expect(empty2.isEmpty)
    }

    @Test("Zero and negative weights are ignored")
    func testZeroAndNegativeWeights() {
        let items = [
            TreemapItem(id: "1", weight: 100),
            TreemapItem(id: "zero", weight: 0),
            TreemapItem(id: "neg", weight: -50),
            TreemapItem(id: "nan", weight: Double.nan),
            TreemapItem(id: "2", weight: 200)
        ]
        let bounds = CGRect(x: 0, y: 0, width: 600, height: 400)
        let rects = SquarifiedTreemapLayout.layout(items: items, bounds: bounds)

        #expect(rects.count == 2)
        #expect(rects.map(\.id).contains("1"))
        #expect(rects.map(\.id).contains("2"))
        #expect(!rects.map(\.id).contains("zero"))
        #expect(!rects.map(\.id).contains("neg"))
    }

    @Test("All rectangles fit within bounds without negative dimensions or NaNs")
    func testBoundsAndNonNegativeDimensions() {
        let items = (1...20).map { TreemapItem(id: "node_\($0)", weight: Double($0 * 50)) }
        let bounds = CGRect(x: 10, y: 20, width: 1024, height: 768)
        let rects = SquarifiedTreemapLayout.layout(items: items, bounds: bounds)

        #expect(rects.count == 20)

        for r in rects {
            #expect(!r.rect.origin.x.isNaN)
            #expect(!r.rect.origin.y.isNaN)
            #expect(!r.rect.size.width.isNaN)
            #expect(!r.rect.size.height.isNaN)

            #expect(r.rect.width >= 0)
            #expect(r.rect.height >= 0)

            #expect(r.rect.minX >= bounds.minX - 0.001)
            #expect(r.rect.minY >= bounds.minY - 0.001)
            #expect(r.rect.maxX <= bounds.maxX + 0.001)
            #expect(r.rect.maxY <= bounds.maxY + 0.001)
        }
    }

    @Test("Total covered area equals container area within epsilon")
    func testTotalAreaPreserved() {
        let items = [
            TreemapItem(id: "A", weight: 500),
            TreemapItem(id: "B", weight: 300),
            TreemapItem(id: "C", weight: 150),
            TreemapItem(id: "D", weight: 50)
        ]
        let bounds = CGRect(x: 0, y: 0, width: 800, height: 600)
        let rects = SquarifiedTreemapLayout.layout(items: items, bounds: bounds)

        let containerArea = bounds.width * bounds.height
        let totalRectArea = rects.reduce(0.0) { $0 + ($1.rect.width * $1.rect.height) }

        #expect(abs(totalRectArea - containerArea) < 0.01)
    }

    @Test("Area ratios strictly match input weight ratios")
    func testAreaRatiosMatchWeightRatios() {
        let items = [
            TreemapItem(id: "Large", weight: 600),
            TreemapItem(id: "Small", weight: 200)
        ]
        let bounds = CGRect(x: 0, y: 0, width: 400, height: 400)
        let rects = SquarifiedTreemapLayout.layout(items: items, bounds: bounds)

        let largeRect = rects.first(where: { $0.id == "Large" })!.rect
        let smallRect = rects.first(where: { $0.id == "Small" })!.rect

        let largeArea = largeRect.width * largeRect.height
        let smallArea = smallRect.width * smallRect.height

        let ratio = largeArea / smallArea
        #expect(abs(ratio - 3.0) < 0.001)
    }

    @Test("Deterministic results for same input")
    func testDeterministicLayout() {
        let items = [
            TreemapItem(id: "3", weight: 300),
            TreemapItem(id: "1", weight: 100),
            TreemapItem(id: "2", weight: 200)
        ]
        let bounds = CGRect(x: 0, y: 0, width: 500, height: 300)

        let run1 = SquarifiedTreemapLayout.layout(items: items, bounds: bounds)
        let run2 = SquarifiedTreemapLayout.layout(items: items, bounds: bounds)

        #expect(run1 == run2)
    }

    @Test("No overlapping rectangles beyond epsilon")
    func testNoOverlap() {
        let items = (1...15).map { TreemapItem(id: "\($0)", weight: Double($0 * 10)) }
        let bounds = CGRect(x: 0, y: 0, width: 800, height: 600)
        let rects = SquarifiedTreemapLayout.layout(items: items, bounds: bounds)

        for i in 0..<rects.count {
            for j in (i + 1)..<rects.count {
                let r1 = rects[i].rect
                let r2 = rects[j].rect
                let intersection = r1.intersection(r2)
                if !intersection.isNull && !intersection.isEmpty {
                    // Small overlap allowed only along borders due to float rounding
                    let overlapArea = intersection.width * intersection.height
                    #expect(overlapArea < 0.01)
                }
            }
        }
    }
}
