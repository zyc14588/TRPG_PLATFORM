// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package archive

import (
	"bytes"
	"encoding/binary"
	"fmt"
	"sort"
	"unicode/utf8"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/packagepath"
)

const (
	localHeaderSignature    = 0x04034b50
	centralHeaderSignature  = 0x02014b50
	endHeaderSignature      = 0x06054b50
	zip64LocatorSignature   = 0x07064b50
	dataDescriptorSignature = 0x08074b50

	localHeaderBytes   = 30
	centralHeaderBytes = 46
	endHeaderBytes     = 22
	maxZIPCommentBytes = 1<<16 - 1

	zipStore   = 0
	zipDeflate = 8
	zip64Extra = 0x0001
)

type zipEntryMetadata struct {
	name             string
	flags            uint16
	method           uint16
	crc32            uint32
	compressedSize   uint64
	uncompressedSize uint64
	localOffset      uint64
	dataOffset       uint64
	dataEnd          uint64
	recordEnd        uint64
}

type zipPreflight struct {
	entries       []zipEntryMetadata
	centralOffset uint64
	expandedBytes uint64
}

// preflightZIP32 validates bounded central/local metadata without invoking
// archive/zip. This ordering prevents a forged central directory from causing
// library allocation before the package limits and portable path tree apply.
func preflightZIP32(data []byte) (zipPreflight, error) {
	endOffset, err := findEndHeader(data)
	if err != nil {
		return zipPreflight{}, err
	}
	end := data[endOffset : endOffset+endHeaderBytes]
	disk := binary.LittleEndian.Uint16(end[4:6])
	centralDisk := binary.LittleEndian.Uint16(end[6:8])
	diskEntries := binary.LittleEndian.Uint16(end[8:10])
	totalEntries := binary.LittleEndian.Uint16(end[10:12])
	centralSize32 := binary.LittleEndian.Uint32(end[12:16])
	centralOffset32 := binary.LittleEndian.Uint32(end[16:20])
	if disk != 0 || centralDisk != 0 || diskEntries != totalEntries {
		return zipPreflight{}, fmt.Errorf("archive must be a single-disk ZIP")
	}
	if totalEntries == ^uint16(0) || centralSize32 == ^uint32(0) || centralOffset32 == ^uint32(0) {
		return zipPreflight{}, fmt.Errorf("ZIP64 archives are not supported")
	}
	if int(totalEntries) > MaxArchiveEntries {
		return zipPreflight{}, fmt.Errorf("archive has %d entries, maximum is %d", totalEntries, MaxArchiveEntries)
	}
	if endOffset >= 20 && binary.LittleEndian.Uint32(data[endOffset-20:endOffset-16]) == zip64LocatorSignature {
		return zipPreflight{}, fmt.Errorf("ZIP64 locator is forbidden")
	}
	centralOffset := uint64(centralOffset32)
	centralSize := uint64(centralSize32)
	if centralSize > MaxCentralDirectoryBytes {
		return zipPreflight{}, fmt.Errorf("central directory exceeds %d bytes", MaxCentralDirectoryBytes)
	}
	if centralOffset+centralSize != uint64(endOffset) || centralOffset > uint64(len(data)) {
		return zipPreflight{}, fmt.Errorf("central directory bounds are inconsistent")
	}
	metadataBytes := centralSize + uint64(binary.LittleEndian.Uint16(end[20:22]))
	if metadataBytes > MaxArchiveMetadataBytes {
		return zipPreflight{}, fmt.Errorf("archive metadata exceeds %d bytes", MaxArchiveMetadataBytes)
	}

	result := zipPreflight{
		entries:       make([]zipEntryMetadata, 0, int(totalEntries)),
		centralOffset: centralOffset,
	}
	var paths packagepath.TreeSet
	position := centralOffset
	for index := 0; index < int(totalEntries); index++ {
		if position+centralHeaderBytes > uint64(endOffset) || binary.LittleEndian.Uint32(data[position:position+4]) != centralHeaderSignature {
			return zipPreflight{}, fmt.Errorf("central directory entry %d is truncated or invalid", index)
		}
		header := data[position : position+centralHeaderBytes]
		versionMadeBy := binary.LittleEndian.Uint16(header[4:6])
		flags := binary.LittleEndian.Uint16(header[8:10])
		method := binary.LittleEndian.Uint16(header[10:12])
		crc := binary.LittleEndian.Uint32(header[16:20])
		compressed := binary.LittleEndian.Uint32(header[20:24])
		expanded := binary.LittleEndian.Uint32(header[24:28])
		nameLength := binary.LittleEndian.Uint16(header[28:30])
		extraLength := binary.LittleEndian.Uint16(header[30:32])
		commentLength := binary.LittleEndian.Uint16(header[32:34])
		diskStart := binary.LittleEndian.Uint16(header[34:36])
		externalAttributes := binary.LittleEndian.Uint32(header[38:42])
		localOffset := binary.LittleEndian.Uint32(header[42:46])
		variableLength := uint64(nameLength) + uint64(extraLength) + uint64(commentLength)
		if nameLength == 0 || int(nameLength) > packagepath.MaxBytes {
			return zipPreflight{}, fmt.Errorf("archive entry %d name exceeds the portable path bound", index)
		}
		if position+centralHeaderBytes+variableLength > uint64(endOffset) {
			return zipPreflight{}, fmt.Errorf("central directory entry %d variable fields are truncated", index)
		}
		nameStart := position + centralHeaderBytes
		nameBytes := data[nameStart : nameStart+uint64(nameLength)]
		extra := data[nameStart+uint64(nameLength) : nameStart+uint64(nameLength)+uint64(extraLength)]
		if !utf8.Valid(nameBytes) {
			return zipPreflight{}, fmt.Errorf("archive entry %d name is empty or invalid UTF-8", index)
		}
		name := string(nameBytes)
		allowedFlags := uint16(0x0008 | 0x0800)
		if method == zipDeflate {
			allowedFlags |= 0x0006 // Deflate compression-option bits.
		}
		if flags&^allowedFlags != 0 {
			return zipPreflight{}, fmt.Errorf("archive entry %q uses unsupported ZIP flags 0x%04x", boundedPath(name), flags)
		}
		if containsNonASCII(nameBytes) && flags&0x0800 == 0 {
			return zipPreflight{}, fmt.Errorf("archive entry %q does not declare its UTF-8 name", boundedPath(name))
		}
		if method != zipStore && method != zipDeflate {
			return zipPreflight{}, fmt.Errorf("archive entry %q uses unsupported compression method %d", boundedPath(name), method)
		}
		if method == zipStore && compressed != expanded {
			return zipPreflight{}, fmt.Errorf("stored archive entry %q has different compressed and expanded sizes", boundedPath(name))
		}
		if diskStart != 0 {
			return zipPreflight{}, fmt.Errorf("archive entry %q starts on another disk", boundedPath(name))
		}
		if err := rejectZIP64Extra(extra); err != nil {
			return zipPreflight{}, fmt.Errorf("archive entry %q: %w", boundedPath(name), err)
		}
		if isSpecialExternalMode(versionMadeBy, externalAttributes) {
			return zipPreflight{}, fmt.Errorf("archive entry %q is a directory, symlink, or special file", boundedPath(name))
		}
		if err := paths.AddFile(name); err != nil {
			return zipPreflight{}, fmt.Errorf("archive entry path: %w", err)
		}
		if uint64(expanded) > MaxEntryExpandedBytes {
			return zipPreflight{}, fmt.Errorf("archive entry %q expands past %d bytes", boundedPath(name), MaxEntryExpandedBytes)
		}
		if expanded > 0 && (compressed == 0 || uint64(expanded) > uint64(compressed)*MaxCompressionRatio) {
			return zipPreflight{}, fmt.Errorf("archive entry %q exceeds %d:1 compression ratio", boundedPath(name), MaxCompressionRatio)
		}
		result.expandedBytes += uint64(expanded)
		if result.expandedBytes > MaxArchiveExpandedBytes {
			return zipPreflight{}, fmt.Errorf("archive expands past %d bytes", MaxArchiveExpandedBytes)
		}
		entry := zipEntryMetadata{
			name: name, flags: flags, method: method, crc32: crc,
			compressedSize: uint64(compressed), uncompressedSize: uint64(expanded), localOffset: uint64(localOffset),
		}
		localMetadata, err := validateLocalHeader(data, centralOffset, &entry)
		if err != nil {
			return zipPreflight{}, fmt.Errorf("archive entry %q: %w", boundedPath(name), err)
		}
		metadataBytes += localMetadata
		if metadataBytes > MaxArchiveMetadataBytes {
			return zipPreflight{}, fmt.Errorf("archive metadata exceeds %d bytes", MaxArchiveMetadataBytes)
		}
		result.entries = append(result.entries, entry)
		position += centralHeaderBytes + variableLength
	}
	if position != uint64(endOffset) {
		return zipPreflight{}, fmt.Errorf("central directory size or entry count is inconsistent")
	}
	if err := validateLocalLayout(result.entries, centralOffset); err != nil {
		return zipPreflight{}, err
	}
	return result, nil
}

func findEndHeader(data []byte) (int, error) {
	if len(data) < endHeaderBytes {
		return 0, fmt.Errorf("archive is too short for a ZIP end record")
	}
	start := len(data) - endHeaderBytes - maxZIPCommentBytes
	if start < 0 {
		start = 0
	}
	for offset := len(data) - endHeaderBytes; offset >= start; offset-- {
		if binary.LittleEndian.Uint32(data[offset:offset+4]) != endHeaderSignature {
			continue
		}
		commentLength := int(binary.LittleEndian.Uint16(data[offset+20 : offset+22]))
		end := offset + endHeaderBytes + commentLength
		if end > len(data) {
			// archive/zip stops at the last signature when its comment is
			// truncated. Do not continue to an earlier record and diverge.
			return 0, fmt.Errorf("ZIP end record comment is truncated")
		}
		if end != len(data) {
			return 0, fmt.Errorf("ZIP end record is followed by unclaimed bytes")
		}
		return offset, nil
	}
	return 0, fmt.Errorf("archive has no exact ZIP32 end record")
}

func validateLocalHeader(data []byte, centralOffset uint64, entry *zipEntryMetadata) (uint64, error) {
	if entry.localOffset+localHeaderBytes > centralOffset || entry.localOffset+localHeaderBytes > uint64(len(data)) {
		return 0, fmt.Errorf("local header is outside the file-data region")
	}
	header := data[entry.localOffset : entry.localOffset+localHeaderBytes]
	if binary.LittleEndian.Uint32(header[0:4]) != localHeaderSignature {
		return 0, fmt.Errorf("local header signature is invalid")
	}
	flags := binary.LittleEndian.Uint16(header[6:8])
	method := binary.LittleEndian.Uint16(header[8:10])
	crc := binary.LittleEndian.Uint32(header[14:18])
	compressed := binary.LittleEndian.Uint32(header[18:22])
	expanded := binary.LittleEndian.Uint32(header[22:26])
	nameLength := binary.LittleEndian.Uint16(header[26:28])
	extraLength := binary.LittleEndian.Uint16(header[28:30])
	if int(nameLength) != len(entry.name) || int(nameLength) > packagepath.MaxBytes {
		return 0, fmt.Errorf("local entry name length differs from the bounded central name")
	}
	if flags != entry.flags || method != entry.method {
		return 0, fmt.Errorf("local and central compression metadata differ")
	}
	variableEnd := entry.localOffset + localHeaderBytes + uint64(nameLength) + uint64(extraLength)
	if variableEnd > centralOffset || variableEnd > uint64(len(data)) {
		return 0, fmt.Errorf("local header variable fields are truncated")
	}
	nameStart := entry.localOffset + localHeaderBytes
	if !bytes.Equal(data[nameStart:nameStart+uint64(nameLength)], []byte(entry.name)) {
		return 0, fmt.Errorf("local and central names differ")
	}
	extra := data[nameStart+uint64(nameLength) : variableEnd]
	if err := rejectZIP64Extra(extra); err != nil {
		return 0, err
	}
	if flags&0x0008 == 0 {
		if crc != entry.crc32 || uint64(compressed) != entry.compressedSize || uint64(expanded) != entry.uncompressedSize {
			return 0, fmt.Errorf("local and central sizes or checksum differ")
		}
	} else {
		if compressed == ^uint32(0) || expanded == ^uint32(0) {
			return 0, fmt.Errorf("ZIP64 local sizes are forbidden")
		}
		if (crc != 0 && crc != entry.crc32) || (compressed != 0 && uint64(compressed) != entry.compressedSize) || (expanded != 0 && uint64(expanded) != entry.uncompressedSize) {
			return 0, fmt.Errorf("local and central sizes or checksum differ")
		}
	}
	entry.dataOffset = variableEnd
	entry.dataEnd = variableEnd + entry.compressedSize
	if entry.dataEnd > centralOffset || entry.dataEnd < variableEnd {
		return 0, fmt.Errorf("compressed data is outside the file-data region")
	}
	entry.recordEnd = entry.dataEnd
	if flags&0x0008 != 0 {
		if err := validateDataDescriptor(data, centralOffset, entry); err != nil {
			return 0, err
		}
	}
	return uint64(nameLength) + uint64(extraLength), nil
}

func validateDataDescriptor(data []byte, centralOffset uint64, entry *zipEntryMetadata) error {
	start := entry.dataEnd
	if start+12 > centralOffset || start+12 > uint64(len(data)) {
		return fmt.Errorf("ZIP32 data descriptor is truncated")
	}
	word0 := binary.LittleEndian.Uint32(data[start : start+4])
	word1 := binary.LittleEndian.Uint32(data[start+4 : start+8])
	word2 := binary.LittleEndian.Uint32(data[start+8 : start+12])
	if word0 == dataDescriptorSignature {
		// archive/zip unconditionally consumes the signature when present.
		// Do not reinterpret the same word as an unsigned descriptor CRC.
		if start+16 > centralOffset || start+16 > uint64(len(data)) {
			return fmt.Errorf("signed ZIP32 data descriptor is truncated")
		}
		crc := binary.LittleEndian.Uint32(data[start+4 : start+8])
		compressed := binary.LittleEndian.Uint32(data[start+8 : start+12])
		expanded := binary.LittleEndian.Uint32(data[start+12 : start+16])
		if crc == entry.crc32 && uint64(compressed) == entry.compressedSize && uint64(expanded) == entry.uncompressedSize {
			entry.recordEnd = start + 16
			return nil
		}
		return fmt.Errorf("signed ZIP32 data descriptor differs from central metadata")
	}
	if word0 == entry.crc32 && uint64(word1) == entry.compressedSize && uint64(word2) == entry.uncompressedSize {
		entry.recordEnd = start + 12
		return nil
	}
	return fmt.Errorf("ZIP32 data descriptor differs from central metadata")
}

func validateLocalLayout(entries []zipEntryMetadata, centralOffset uint64) error {
	if len(entries) == 0 {
		if centralOffset != 0 {
			return fmt.Errorf("empty archive contains a preamble")
		}
		return nil
	}
	ordered := append([]zipEntryMetadata(nil), entries...)
	sort.Slice(ordered, func(i, j int) bool { return ordered[i].localOffset < ordered[j].localOffset })
	if ordered[0].localOffset != 0 {
		return fmt.Errorf("archive contains data before the first local header")
	}
	for index := 1; index < len(ordered); index++ {
		if ordered[index].localOffset != ordered[index-1].recordEnd {
			return fmt.Errorf("archive local records have a gap or overlap")
		}
	}
	if ordered[len(ordered)-1].recordEnd != centralOffset {
		return fmt.Errorf("archive local records do not exactly reach the central directory")
	}
	return nil
}

func rejectZIP64Extra(extra []byte) error {
	for len(extra) != 0 {
		if len(extra) < 4 {
			return fmt.Errorf("ZIP extra field is truncated")
		}
		identifier := binary.LittleEndian.Uint16(extra[0:2])
		length := int(binary.LittleEndian.Uint16(extra[2:4]))
		extra = extra[4:]
		if length > len(extra) {
			return fmt.Errorf("ZIP extra field is truncated")
		}
		if identifier == zip64Extra {
			return fmt.Errorf("ZIP64 extra field is forbidden")
		}
		extra = extra[length:]
	}
	return nil
}

func isSpecialExternalMode(versionMadeBy uint16, attributes uint32) bool {
	if attributes&0x18 != 0 { // DOS volume-label or directory bit.
		return true
	}
	creator := versionMadeBy >> 8
	if creator != 3 && creator != 19 { // Unix and macOS use Unix mode in the high 16 bits.
		return false
	}
	modeType := (attributes >> 16) & 0170000
	return modeType != 0 && modeType != 0100000
}

func containsNonASCII(value []byte) bool {
	for _, character := range value {
		if character >= utf8.RuneSelf {
			return true
		}
	}
	return false
}

func boundedPath(value string) string {
	const maximum = 96
	if len(value) <= maximum {
		return value
	}
	return value[:maximum] + "..."
}
