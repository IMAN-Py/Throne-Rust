package scan

import (
	"testing"

	"ThroneCore/internal/fastcore"
	"golang.org/x/net/dns/dnsmessage"
)

func parseXDNSReplyGoReference(packet []byte, query xdnsQuery) (xdnsReply, bool) {
	var parser dnsmessage.Parser
	header, err := parser.Start(packet)
	if err != nil || !header.Response || header.ID != query.id {
		return xdnsReply{}, false
	}
	question, err := parser.Question()
	if err != nil {
		return xdnsReply{rcode: header.RCode}, err == dnsmessage.ErrSectionDone && header.RCode != dnsmessage.RCodeSuccess
	}
	if !sameDNSName(question.Name, query.name) {
		return xdnsReply{}, false
	}
	reply := xdnsReply{rcode: header.RCode}
	if parser.SkipAllQuestions() != nil {
		return reply, true
	}
	for {
		answer, err := parser.AnswerHeader()
		if err != nil {
			return reply, true
		}
		if answer.Type == dnsmessage.TypeTXT && answer.Class == dnsmessage.ClassINET && sameDNSName(answer.Name, query.name) {
			reply.txt = true
			return reply, true
		}
		if parser.SkipAnswer() != nil {
			return reply, true
		}
	}
}

func buildTestDNSPacket(id uint16, response bool, rcode dnsmessage.RCode, qName, aName string, withTXT bool, compress bool) []byte {
	hdr := dnsmessage.Header{
		ID:       id,
		Response: response,
		RCode:    rcode,
	}
	b := dnsmessage.NewBuilder(nil, hdr)
	if compress {
		b.EnableCompression()
	}
	if qName != "" {
		_ = b.StartQuestions()
		_ = b.Question(dnsmessage.Question{
			Name:  dnsmessage.MustNewName(qName),
			Type:  dnsmessage.TypeTXT,
			Class: dnsmessage.ClassINET,
		})
	}
	if withTXT && aName != "" {
		_ = b.StartAnswers()
		_ = b.TXTResource(
			dnsmessage.ResourceHeader{
				Name:  dnsmessage.MustNewName(aName),
				Type:  dnsmessage.TypeTXT,
				Class: dnsmessage.ClassINET,
				TTL:   30,
			},
			dnsmessage.TXTResource{TXT: []string{"hello-xdns"}},
		)
	}
	out, _ := b.Finish()
	return out
}

func TestParseXDNSReplyDifferential(t *testing.T) {
	if !fastcore.Available() {
		t.Fatalf("throne_fast_core.dll not loaded")
	}

	qName := dnsmessage.MustNewName("a1b2c3d4e5.xdns.example.com.")
	q := xdnsQuery{id: 0x3A7F, name: qName}

	wireLabels := func(labels ...string) []byte {
		var b []byte
		for _, lbl := range labels {
			b = append(b, byte(len(lbl)))
			b = append(b, lbl...)
		}
		return append(b, 0)
	}
	qWire := wireLabels("a1b2c3d4e5", "xdns", "example", "com")
	makeQuestionChain := func(hops int, answerFollows bool) []byte {
		// Header: ID=0x3A7F, Flags=0x8000 (QR=1), QD=1, AN=0 or 1
		an := byte(0)
		if answerFollows {
			an = 1
		}
		pkt := []byte{0x3A, 0x7F, 0x80, 0x00, 0x00, 0x01, 0x00, an, 0x00, 0x00, 0x00, 0x00}
		// Offset 12: pointer to 18 (after QTYPE/QCLASS at 14..17)
		pkt = append(pkt, 0xC0, 18, 0x00, 16, 0x00, 0x01)
		// Chain remaining (hops - 1) pointers starting at offset 18
		for i := 1; i < hops; i++ {
			nextOff := byte(len(pkt) + 2)
			pkt = append(pkt, 0xC0, nextOff)
		}
		wireOff := len(pkt)
		pkt = append(pkt, qWire...)
		if answerFollows {
			// Answer pointing directly to wireOff, TXT IN TTL=60 RDLEN=1 ""
			pkt = append(pkt, 0xC0|byte(wireOff>>8), byte(wireOff), 0x00, 16, 0x00, 0x01, 0, 0, 0, 60, 0, 1, 0)
		}
		return pkt
	}
	makeAnswerWithOtherRecordFirst := func(otherWireName []byte) []byte {
		// QD=1 (qWire TXT), AN=2 (otherWireName A 1.2.3.4, then qName TXT)
		pkt := []byte{0x3A, 0x7F, 0x80, 0x00, 0x00, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00}
		pkt = append(pkt, qWire...)
		pkt = append(pkt, 0x00, 16, 0x00, 0x01)
		pkt = append(pkt, otherWireName...)
		pkt = append(pkt, 0x00, 0x01, 0x00, 0x01, 0, 0, 0, 60, 0, 4, 1, 2, 3, 4)
		pkt = append(pkt, 0xC0, 12, 0x00, 16, 0x00, 0x01, 0, 0, 0, 60, 0, 1, 0)
		return pkt
	}
	makeNameLen := func(targetUnpackedLen int) []byte {
		// Each label N contributes (N + 1) unpacked bytes including its dot.
		var labels []string
		rem := targetUnpackedLen
		for rem > 0 {
			chunk := 63
			if rem-1 < chunk {
				chunk = rem - 1
			}
			lbl := make([]byte, chunk)
			for i := range lbl {
				lbl[i] = 'a'
			}
			labels = append(labels, string(lbl))
			rem -= chunk + 1
		}
		return wireLabels(labels...)
	}

	multiQCorruptQ2 := []byte{0x3A, 0x7F, 0x80, 0x00, 0x00, 0x02, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00}
	multiQCorruptQ2 = append(multiQCorruptQ2, qWire...)
	multiQCorruptQ2 = append(multiQCorruptQ2, 0x00, 16, 0x00, 0x01, 0xC0, 0xFF, 0x00, 16, 0x00, 0x01)
	multiQCorruptQ2 = append(multiQCorruptQ2, 0xC0, 12, 0x00, 16, 0x00, 0x01, 0, 0, 0, 60, 0, 1, 0)

	ansLoop := []byte{0x3A, 0x7F, 0x80, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00}
	ansLoop = append(ansLoop, qWire...)
	ansLoop = append(ansLoop, 0x00, 16, 0x00, 0x01)
	loopOff := byte(len(ansLoop))
	ansLoop = append(ansLoop, 0xC0, loopOff, 0x00, 16, 0x00, 0x01, 0, 0, 0, 60, 0, 1, 0)

	truncTxtRdata := []byte{0x3A, 0x7F, 0x80, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00}
	truncTxtRdata = append(truncTxtRdata, qWire...)
	truncTxtRdata = append(truncTxtRdata, 0x00, 16, 0x00, 0x01, 0xC0, 12, 0x00, 16, 0x00, 0x01, 0, 0, 0, 60, 0, 5, 1, 'x')

	truncNonTxtRdata := []byte{0x3A, 0x7F, 0x80, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00}
	truncNonTxtRdata = append(truncNonTxtRdata, qWire...)
	truncNonTxtRdata = append(truncNonTxtRdata, 0x00, 16, 0x00, 0x01, 0xC0, 12, 0x00, 0x01, 0x00, 0x01, 0, 0, 0, 60, 0, 4, 1, 2)

	cases := []struct {
		name string
		pkt  []byte
	}{
		{"valid_compressed_txt", buildTestDNSPacket(0x3A7F, true, dnsmessage.RCodeSuccess, "a1b2c3d4e5.xdns.example.com.", "a1b2c3d4e5.xdns.example.com.", true, true)},
		{"valid_0x20_case_randomized", buildTestDNSPacket(0x3A7F, true, dnsmessage.RCodeSuccess, "A1b2C3d4e5.XdNs.ExAmPlE.cOm.", "a1b2c3D4E5.xdns.example.COM.", true, false)},
		{"wrong_id", buildTestDNSPacket(0x1111, true, dnsmessage.RCodeSuccess, "a1b2c3d4e5.xdns.example.com.", "a1b2c3d4e5.xdns.example.com.", true, true)},
		{"not_a_response", buildTestDNSPacket(0x3A7F, false, dnsmessage.RCodeSuccess, "a1b2c3d4e5.xdns.example.com.", "", false, false)},
		{"nxdomain_no_question", buildTestDNSPacket(0x3A7F, true, dnsmessage.RCodeNameError, "", "", false, false)},
		{"noerror_no_question", buildTestDNSPacket(0x3A7F, true, dnsmessage.RCodeSuccess, "", "", false, false)},
		{"noerror_empty_answer", buildTestDNSPacket(0x3A7F, true, dnsmessage.RCodeSuccess, "a1b2c3d4e5.xdns.example.com.", "", false, false)},
		{"mismatched_question_name", buildTestDNSPacket(0x3A7F, true, dnsmessage.RCodeSuccess, "other.example.com.", "a1b2c3d4e5.xdns.example.com.", true, false)},
		{"mismatched_answer_name", buildTestDNSPacket(0x3A7F, true, dnsmessage.RCodeSuccess, "a1b2c3d4e5.xdns.example.com.", "other.example.com.", true, false)},
		{"truncated_short", []byte{0x3A, 0x7F, 0x80}},
		{"compression_loop", []byte{0x3A, 0x7F, 0x80, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0, 0xC0, 12, 0, 16, 0, 1}},
		{"oob_pointer", []byte{0x3A, 0x7F, 0x80, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0, 0xC0, 0xFF, 0, 16, 0, 1}},
		{"exactly_10_pointer_hops", makeQuestionChain(10, true)},
		{"reject_11_pointer_hops", makeQuestionChain(11, true)},
		{"dot_in_label_answer", makeAnswerWithOtherRecordFirst(wireLabels("a.b", "com"))},
		{"max_254_byte_name_ok", makeAnswerWithOtherRecordFirst(makeNameLen(254))},
		{"reject_255_byte_name", makeAnswerWithOtherRecordFirst(makeNameLen(255))},
		{"multi_question_corrupt_q2", multiQCorruptQ2},
		{"answer_compression_loop", ansLoop},
		{"truncated_txt_rdata", truncTxtRdata},
		{"truncated_non_txt_rdata", truncNonTxtRdata},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			gotReply, gotMatch := parseXDNSReply(tc.pkt, q)
			wantReply, wantMatch := parseXDNSReplyGoReference(tc.pkt, q)
			if gotMatch != wantMatch || gotReply != wantReply {
				t.Fatalf("mismatch for %s: rust=(%+v, %v) go=(%+v, %v)", tc.name, gotReply, gotMatch, wantReply, wantMatch)
			}
		})
	}
}

func BenchmarkParseXDNSReplyRust(b *testing.B) {
	if !fastcore.Available() {
		b.Skip("throne_fast_core.dll not loaded")
	}
	qName := dnsmessage.MustNewName("a1b2c3d4e5.xdns.example.com.")
	q := xdnsQuery{id: 0x3A7F, name: qName}
	pkt := buildTestDNSPacket(0x3A7F, true, dnsmessage.RCodeSuccess, "a1b2c3d4e5.xdns.example.com.", "A1B2c3d4e5.XDNS.example.com.", true, true)
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		r, ok := parseXDNSReply(pkt, q)
		if !ok || !r.txt {
			b.Fatal("unexpected mismatch")
		}
	}
}

func BenchmarkParseXDNSReplyGo(b *testing.B) {
	qName := dnsmessage.MustNewName("a1b2c3d4e5.xdns.example.com.")
	q := xdnsQuery{id: 0x3A7F, name: qName}
	pkt := buildTestDNSPacket(0x3A7F, true, dnsmessage.RCodeSuccess, "a1b2c3d4e5.xdns.example.com.", "A1B2c3d4e5.XDNS.example.com.", true, true)
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		r, ok := parseXDNSReplyGoReference(pkt, q)
		if !ok || !r.txt {
			b.Fatal("unexpected mismatch")
		}
	}
}
